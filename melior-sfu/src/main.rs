mod structs;
mod shared;
mod strategies;
mod streaming;
mod prelude;
mod error;
mod routes;

use std::sync::Arc;
use fred::error::{Error};
use tokio::signal;
use tokio::sync::Mutex;
use crate::shared::redis_manager::RedisManager;
use crate::shared::webrtc_manager::WebRtcManager;
use crate::structs::app_state::AppState;
use color_eyre::Result;
use axum::{
    extract::ws::{WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Router,
};

async fn ws_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_socket)
}

async fn handle_socket(mut socket: WebSocket) {
    while let Some(msg) = socket.recv().await {
        let msg = if let Ok(msg) = msg {
            msg
        } else {
            println!("Unexpected error during receive");
            // client disconnected
            return;
        };

        if socket.send(msg).await.is_err() {
            println!("Unexpected error during sent:");

            // client disconnected
            return;
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let app = Router::new()
        .route("/ws", get(ws_handler));
    let listener = tokio::net::TcpListener::bind("localhost:5000").await?;
    axum::serve(listener, app).await?;
    //
    // let manager = Arc::new(RedisManager::new("127.0.0.1", 6379));
    // let web_rtc_manager = Arc::new(Mutex::new(WebRtcManager::new()));
    //
    // let app_state = Arc::new(AppState {
    //     redis_manager: manager.clone(),
    //     web_rtc_manager: web_rtc_manager.clone(),
    // });
    //
    // manager.init().await?;
    // manager.register_handlers(app_state).await;
    //
    // // Wait for Ctrl+C
    // signal::ctrl_c().await.expect("Failed to listen for event");
    //
    // println!("Shutting down...");
    // // Graceful shutdown
    // manager.disconnect().await?;

    Ok(())
}