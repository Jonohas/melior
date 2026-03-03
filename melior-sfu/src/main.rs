mod error;
mod handlers;
mod prelude;
mod shared;
mod streaming;
mod structs;
mod websocket;

use crate::shared::webrtc_manager::WebRtcManager;
use crate::structs::app_state::AppState;
use crate::structs::client_registry::ClientRegistry;
use crate::websocket::handler::ws_handler;
use axum::routing::get;
use axum::Router;
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() {
    let client_registry = Arc::new(ClientRegistry::new());

    let app_state = AppState {
        web_rtc_manager: Arc::new(Mutex::new(WebRtcManager::new())),
        client_registry,
    };

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:5000")
        .await
        .unwrap();

    axum::serve(listener, app).await.unwrap();
}
