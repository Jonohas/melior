use crate::handlers::websocket_strategy::WebsocketStrategy;
use crate::structs::app_state::AppState;
use axum::extract::ws::{Message, WebSocket};

pub struct EchoStrategy;

impl WebsocketStrategy for EchoStrategy {
    type Message = String;

    fn new() -> Self {
        println!("Echo handler initialized");
        Self
    }

    async fn handle_message(
        &self,
        socket: &mut WebSocket,
        message: Self::Message,
        _session_id: &str,
        _channel_id: &str,
        _state: &AppState,
    ) {
        println!("Echo: {}", message);
        let _ = socket.send(Message::text(message)).await;
    }
}
