use axum::extract::ws::{Message, WebSocket};
use serde::Deserialize;
use crate::handlers::websocket_strategy::WebsocketStrategy;

#[derive(Deserialize)]
pub struct SDPOfferMessage {
    sdp: String,
}

pub struct AnswerSDPOfferStrategy;

impl WebsocketStrategy for AnswerSDPOfferStrategy {
    type Message = SDPOfferMessage;

    fn new() -> Self {
        println!("Sdp handler initialized");
        Self
    }

    async fn handle_message(&self, socket: &mut WebSocket, message: Self::Message) {
        println!("Handle sdp offer: {}", message.sdp);
        let _ = socket.send(Message::text(message.sdp)).await;
    }
}