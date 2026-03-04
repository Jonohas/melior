use crate::handlers::websocket_strategy::WebsocketStrategy;
use crate::structs::app_state::AppState;
use axum::extract::ws::{Message, WebSocket};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct SDPOfferMessage {
    sdp: String,
    #[serde(rename = "type")]
    sdp_type: String, // This will capture "offer"
}

pub struct AnswerSDPOfferStrategy;

impl WebsocketStrategy for AnswerSDPOfferStrategy {
    type Message = SDPOfferMessage;

    fn new() -> Self {
        println!("Sdp handler initialized");
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
        let answer_sdp = state
            .web_rtc_manager
            .lock()
            .await
            .create_response_to_offer(
                message.sdp,
                session_id.to_string(),
                state.client_registry.clone(),
                state.track_registry.clone(),
            )
            .await;

        match answer_sdp {
            Ok(sdp) => {
                let response = serde_json::json!({ "channel": "sdp_answer", "payload": {
                    "sdp": sdp,
                    "type": "answer"
                }});
                let _ = socket
                    .send(Message::text(serde_json::to_string(&response).unwrap()))
                    .await;
            }
            Err(e) => {
                eprintln!("Failed to create answer: {}", e);
            }
        }
    }
}
