use axum::extract::ws::{Message, WebSocket};
use crate::handlers::websocket_strategy::WebsocketStrategy;

pub struct EchoStrategy;

impl WebsocketStrategy for EchoStrategy {
    type Message = String;

    fn new() -> Self {
        println!("Echo handler initialized");
        Self
    }

    async fn handle_message(&self, socket: &mut WebSocket, message: Self::Message) {
        println!("Echo: {}", message);
        let _ = socket.send(Message::text(message)).await;
    }
}