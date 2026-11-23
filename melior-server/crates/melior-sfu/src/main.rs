//! The Melior SFU server.
//!
//! This crate is the composition root: it reads the configuration, constructs the
//! adapters, hands them to the application services and serves the API. It is the
//! only crate that knows every layer, and it holds no logic of its own.

#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

use melior_application::{
    AuthService, AuthSettings, ChatService, CommunityService, ConferenceService, ImageStore,
    PresenceQueue, UnconfiguredImageStore, start_image_sweeper,
};
use melior_domain::MeshPolicy;
use melior_infrastructure::config::Config;
use melior_infrastructure::oidc::{OidcIdentityProvider, OidcSettings};
use melior_infrastructure::postgres::{
    Database, PostgresChatRepository, PostgresCommunityRepository,
};
use melior_infrastructure::redis::{
    RedisCommunityEvents, RedisConnection, RedisRoomLeases, RedisSessionStore, RedisVoiceRoster,
};
use melior_infrastructure::s3::S3ImageStore;
use melior_infrastructure::telemetry;
use melior_infrastructure::turn::TurnRestCredentials;
use melior_infrastructure::webrtc::WebRtcMediaEngine;
use melior_interfaces::{AppState, router};
use std::error::Error;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let config = Config::from_env()?;
    let telemetry = telemetry::init(config.otlp_endpoint.as_deref(), &config.instance_id)?;

    // Shared by everything that lives in Redis: sessions, room leases, the
    // community event bus and the voice roster.
    let redis = Arc::new(RedisConnection::new(&config.redis_url)?);

    let media = Arc::new(WebRtcMediaEngine::new()?);
    let ice_servers = Arc::new(TurnRestCredentials::new(
        config.turn_urls.clone(),
        config.turn_secret.clone(),
        config.turn_ttl,
    ));
    let leases = Arc::new(RedisRoomLeases::new(
        redis.clone(),
        config.instance_id.clone(),
        config.room_lease_ttl(),
    ));
    let mesh_limit = config.effective_mesh_limit();
    if config.mesh_max_participants > 0 && mesh_limit == 0 {
        info!("mesh is off: no TURN server is configured (MELIOR_TURN_URLS)");
    }
    let mesh = MeshPolicy::new(mesh_limit);
    let events = Arc::new(RedisCommunityEvents::new(redis.clone()));
    let roster = Arc::new(RedisVoiceRoster::new(redis.clone()));
    // A roster entry lives as long as a room lease: three re-checks.
    let presence = PresenceQueue::start(roster.clone(), events.clone(), config.room_lease_ttl());
    let conference = ConferenceService::new(media, ice_servers, leases, mesh, presence);

    let identity_provider = Arc::new(OidcIdentityProvider::new(OidcSettings {
        issuer: config.idp_issuer.clone(),
        client_id: config.idp_client_id.clone(),
        scope: config.idp_scope.clone(),
    })?);
    let auth = Arc::new(AuthService::new(
        identity_provider,
        Arc::new(RedisSessionStore::new(redis)),
        AuthSettings::new(&config.frontend_url, config.session_ttl, config.token_refresh_lead),
    ));

    // One lazily connected pool for everything stored in Postgres.
    let database = Arc::new(Database::new(&config.database_url)?);
    let communities = Arc::new(CommunityService::new(
        Arc::new(PostgresCommunityRepository::new(database.clone())),
        events,
        roster,
        mesh,
    ));
    // Without S3 settings image uploads are refused; everything else works.
    let images: Arc<dyn ImageStore> = match config.s3.clone() {
        Some(settings) => Arc::new(S3ImageStore::new(settings)?),
        None => Arc::new(UnconfiguredImageStore),
    };
    let chat = Arc::new(ChatService::new(
        conference.clone(),
        Arc::new(PostgresChatRepository::new(database)),
        images,
        communities.clone(),
    ));

    // Images uploaded for messages that were never sent.
    start_image_sweeper(chat.clone(), config.unattached_image_ttl, config.image_sweep_interval());

    let app = router(AppState {
        conference,
        chat,
        auth,
        communities,
        session_cookie_name: config.session_cookie_name.as_str().into(),
        session_recheck: config.session_recheck,
        klipy_app_key: config.klipy_app_key.as_deref().map(Arc::from),
    });

    let listener = TcpListener::bind(config.bind).await?;
    info!(
        address = %listener.local_addr()?,
        instance = %config.instance_id,
        frontend = %config.frontend_url,
        issuer = %config.idp_issuer,
        traces = config.otlp_endpoint.as_deref().unwrap_or("off"),
        turn_urls = config.turn_urls.len(),
        mesh_limit,
        images = config.s3.as_ref().map_or("off", |s3| s3.endpoint.as_str()),
        gifs = config.klipy_app_key.is_some(),
        "melior-sfu listening"
    );
    // Stopping drops the open connections; clients reconnect by themselves, and
    // with several instances their rooms move to another one.
    tokio::select! {
        result = axum::serve(listener, app) => result?,
        () = shutdown_signal() => info!("shutting down"),
    }
    telemetry.shutdown();
    Ok(())
}

/// Resolves on Ctrl-C or, on Unix, SIGTERM (what `docker stop` sends).
async fn shutdown_signal() {
    let interrupt = async {
        if tokio::signal::ctrl_c().await.is_err() {
            std::future::pending::<()>().await;
        }
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = interrupt => {}
        () = terminate => {}
    }
}
