use axum::extract::ws::WebSocket;
use serde::{Deserialize};
use serde_json::Value;

pub trait WebsocketStrategy {
    // 1. Define the type this strategy expects
    type Message: for<'a> Deserialize<'a>;

    fn new() -> Self;

    async fn handle_message(&self, socket: &mut WebSocket, message: Self::Message);

    async fn check_format(&self, socket: &mut WebSocket, message: Value, channel: String) {
        if let payload = message.to_string() {
            match serde_json::from_str::<Self::Message>(&payload) {
                Ok(parsed) => self.handle_message(socket, parsed).await,
                Err(_) => eprintln!("[Websocket] Schema mismatch on {}", channel),
            }
        }
    }
}
