//! A real server on a random port, with the outside world replaced: an identity
//! provider that accepts anyone and sessions kept in memory.
#![allow(dead_code)]

use async_trait::async_trait;
use melior_application::{
    AuthService, AuthSettings, Authentication, AuthorizationRequest, ConferenceService,
    IdentityProvider, IdpError, InMemoryRoomRepository, InMemorySessionStore, LeaseError,
    RoomAccessService, RoomLeases, StaticIceServers, Tokens,
};
use melior_domain::{ChannelId, User, UserId};
use melior_infrastructure::webrtc::WebRtcMediaEngine;
use melior_interfaces::{AppState, router};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::handshake::client::Request;
use tokio_tungstenite::tungstenite::http::HeaderValue;

pub const SESSION_COOKIE: &str = "melior_session_id";
/// The origin the server believes the application is served from.
pub const FRONTEND: &str = "http://app.test";

/// Vouches for whoever the authorization code names: `"<username>:<role>,<role>"`.
pub struct FakeIdp;

#[async_trait]
impl IdentityProvider for FakeIdp {
    async fn authorization_request(
        &self,
        redirect_uri: &str,
        state: &str,
    ) -> Result<AuthorizationRequest, IdpError> {
        Ok(AuthorizationRequest {
            url: format!("http://idp.test/auth?redirect_uri={redirect_uri}&state={state}"),
            code_verifier: "verifier".into(),
            nonce: "nonce".into(),
        })
    }

    async fn exchange_code(
        &self,
        code: &str,
        code_verifier: &str,
        nonce: &str,
        _redirect_uri: &str,
    ) -> Result<Authentication, IdpError> {
        let (username, roles) = code.split_once(':').ok_or(IdpError::InvalidGrant)?;
        if code_verifier != "verifier" || nonce != "nonce" {
            return Err(IdpError::InvalidGrant);
        }
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        Ok(Authentication {
            user: User::new(
                UserId::new(format!("sub-{username}")),
                username,
                Some(format!("{username}@example.test")),
                roles.split(',').filter(|role| !role.is_empty()).map(str::to_owned),
            ),
            tokens: Tokens {
                access_token: format!("access-{username}"),
                refresh_token: None,
                id_token: Some(format!("id-{username}")),
                access_token_expires_at: now + 3600,
            },
        })
    }

    async fn refresh(&self, _refresh_token: &str) -> Result<Authentication, IdpError> {
        Err(IdpError::InvalidGrant)
    }

    async fn logout_url(
        &self,
        id_token: &str,
        post_logout_redirect_uri: &str,
    ) -> Result<String, IdpError> {
        Ok(format!("http://idp.test/logout?hint={id_token}&to={post_logout_redirect_uri}"))
    }
}

/// Stands in for the lease on Redis: the test decides whether this instance still
/// hosts its rooms.
#[derive(Default)]
pub struct SwitchableLeases {
    hosted_elsewhere: AtomicBool,
}

impl SwitchableLeases {
    /// Simulates another instance taking over every room.
    pub fn move_rooms_elsewhere(&self) {
        self.hosted_elsewhere.store(true, Ordering::SeqCst);
    }
}

#[async_trait]
impl RoomLeases for SwitchableLeases {
    async fn hold(&self, _channel_id: &ChannelId) -> Result<bool, LeaseError> {
        Ok(!self.hosted_elsewhere.load(Ordering::SeqCst))
    }
}

/// How often the test server re-checks open connections.
pub const RECHECK: Duration = Duration::from_millis(150);

pub struct TestServer {
    pub address: SocketAddr,
    pub auth: Arc<AuthService>,
    pub sessions: Arc<InMemorySessionStore>,
    pub rooms: Arc<RoomAccessService>,
    pub leases: Arc<SwitchableLeases>,
}

impl TestServer {
    pub async fn start() -> Self {
        let media = Arc::new(WebRtcMediaEngine::new().expect("media engine"));
        let leases = Arc::new(SwitchableLeases::default());
        let conference =
            ConferenceService::new(media, Arc::new(StaticIceServers::default()), leases.clone());
        let sessions = Arc::new(InMemorySessionStore::default());
        let auth = Arc::new(AuthService::new(
            Arc::new(FakeIdp),
            sessions.clone(),
            AuthSettings::new(FRONTEND, Duration::from_secs(3600), Duration::from_secs(30)),
        ));
        let rooms = Arc::new(RoomAccessService::new(Arc::new(InMemoryRoomRepository::default())));
        let app = router(AppState {
            conference,
            auth: auth.clone(),
            rooms: rooms.clone(),
            session_cookie_name: SESSION_COOKIE.into(),
            session_recheck: RECHECK,
        });

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self { address, auth, sessions, rooms, leases }
    }

    pub fn url(&self, path: &str) -> String {
        format!("http://{}{path}", self.address)
    }

    /// Logs a user in without the HTTP round trip. Returns the `Cookie` header value.
    pub async fn login(&self, username: &str, roles: &[&str]) -> String {
        let code = format!("{username}:{}", roles.join(","));
        let login =
            self.auth.complete_login(&code, None, Some("verifier.nonce")).await.expect("login");
        format!("{SESSION_COOKIE}={}", login.session_id.as_str())
    }

    /// A WebSocket handshake for a room, optionally carrying a session cookie.
    pub fn ws_request(&self, channel: Option<&str>, cookie: Option<&str>) -> Request {
        let query = channel.map(|channel| format!("?channel_id={channel}")).unwrap_or_default();
        let mut request = format!("ws://{}/ws{query}", self.address).into_client_request().unwrap();
        if let Some(cookie) = cookie {
            request.headers_mut().insert("Cookie", HeaderValue::from_str(cookie).unwrap());
        }
        request
    }
}
