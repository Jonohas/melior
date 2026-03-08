use crate::handlers::websocket_strategy::WebsocketStrategy;
use crate::structs::app_state::AppState;
use crate::tui::EventEmitter;
use axum::extract::ws::WebSocket;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Deserialize)]
pub struct SDPAnswerMessage {
    pub sdp: String,
    pub r#type: String,
}

pub struct HandleSDPAnswerStrategy;

impl WebsocketStrategy for HandleSDPAnswerStrategy {
    type Message = SDPAnswerMessage;

    fn new() -> Self {
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
            let emitter = state.event_emitter.lock().await;
            emitter.emit_log(
                "ERROR",
                &format!("Failed to handle renegotiation answer: {}", e),
            );
        }
    }
}
