use crate::handlers::websocket_strategy::WebsocketStrategy;
use crate::structs::app_state::AppState;
use axum::extract::ws::{Message, WebSocket};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct SDPAnswerMessage {
    pub sdp: String,
    pub r#type: String,
}

pub struct HandleSDPAnswerStrategy;

impl WebsocketStrategy for HandleSDPAnswerStrategy {
    type Message = SDPAnswerMessage;

    fn new() -> Self {
        println!("Sdp answer handler initialized");
        Self
    }

    async fn handle_message(
        &self,
        socket: &mut WebSocket,
        message: Self::Message,
        session_id: &str,
        channel_id: &str,
        state: &AppState,
    ) {
        let mut webrtc_manager = state.web_rtc_manager.lock().await;

        if let Err(e) = webrtc_manager
            .handle_renegotiation_answer(session_id, message.sdp)
            .await
        {
            eprintln!("Failed to handle renegotiation answer: {}", e);
        }
    }
}