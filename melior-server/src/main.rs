mod error;
mod handlers;
mod prelude;
mod shared;
mod structs;
mod tui;
mod websocket;

use crate::shared::webrtc_manager::WebRtcManager;
use crate::structs::app_state::AppState;
use crate::structs::client_registry::ClientRegistry;
use crate::tui::EventEmitter;
use crate::websocket::handler::ws_handler;
use axum::routing::get;
use axum::Router;
use clap::Parser;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Parser, Debug)]
#[command(name = "melior-sfu")]
#[command(about = "WebRTC SFU Server", long_about = None)]
struct Args {
    #[arg(long)]
    tui: bool,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let emitter = Arc::new(Mutex::new(EventEmitter::new()));

    let client_registry = Arc::new(ClientRegistry::new());

    let app_state = AppState {
        web_rtc_manager: Arc::new(Mutex::new(
            WebRtcManager::new().with_emitter(emitter.clone()),
        )),
        client_registry,
        track_registry: crate::shared::track_registry::TrackRegistry::new(emitter.clone()),
        event_emitter: emitter.clone(),
    };

    tui::start_socket_server(emitter.clone());

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:5000").await.unwrap();

    let server_handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    {
        let emitter = emitter.lock().await;
        emitter.emit_log("INFO", "Server running on http://0.0.0.0:5000");
        if args.tui {
            emitter.emit_log("INFO", "TUI started - press Ctrl+C to exit");
        }
    }

    if args.tui {
        tui::run_embedded_tui(emitter).await;
    }

    server_handle.await.unwrap();
}
