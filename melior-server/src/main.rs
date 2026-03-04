mod error;
mod handlers;
mod prelude;
mod shared;
mod streaming;
mod structs;
mod websocket;

use std::collections::HashMap;
use crate::shared::webrtc_manager::WebRtcManager;
use crate::structs::app_state::AppState;
use crate::structs::client_registry::ClientRegistry;
use crate::websocket::handler::ws_handler;
use axum::routing::get;
use axum::Router;
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::shared::track_registry::TrackRegistry;
use axum_server::tls_rustls::RustlsConfig;



#[tokio::main]
async fn main() {
    rustls::crypto::ring::default_provider()
        .install_default()
        .unwrap();

    let client_registry = Arc::new(ClientRegistry::new());

    let app_state = AppState {
        web_rtc_manager: Arc::new(Mutex::new(WebRtcManager::new())),
        client_registry,
        track_registry: TrackRegistry {
            tracks: Arc::new(Mutex::new(HashMap::new())),
        },
    };

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .with_state(app_state);

    let config = RustlsConfig::from_pem_file("cert.pem", "key.pem")
        .await
        .unwrap();

    axum_server::bind_rustls(
        "0.0.0.0:5000".parse().unwrap(),
        config,
    )
        .serve(app.into_make_service())
        .await
        .unwrap();
}
