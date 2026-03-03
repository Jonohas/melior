use std::sync::Arc;
use axum::extract::ws::{Message, WebSocket};
use axum::extract::WebSocketUpgrade;
use axum::response::IntoResponse;
use super::message::IncomingMessage;
use super::router::WebsocketMessageRouter;

trait WebsocketMessageHandler {
    fn handle(&self, socket: &mut WebSocket, msg: &IncomingMessage);
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket))
}

async fn handle_socket(mut socket: WebSocket) {
    while let Some(msg) = socket.recv().await {
        let msg = match msg {
            Ok(Message::Text(text)) => text,
            Ok(Message::Close(_)) | Err(_) => return,
            _ => continue,
        };

        let incoming: IncomingMessage = match serde_json::from_str(&msg) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("Invalid message: {e}");
                continue;
            }
        };
        WebsocketMessageRouter::handle(&mut socket, &incoming).await
    }
}