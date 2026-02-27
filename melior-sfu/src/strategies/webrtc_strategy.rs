use std::sync::Arc;
use serde::{Deserialize, Serialize};
use crate::shared::channel_constants::WEBRTC_CHANNEL;
use crate::shared::redis_strategy::RedisStrategy;
use crate::structs::app_state::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct WebrtcRequest {
    pub sdp_type: String,
    pub sdp: String,
    pub session_id: String,
    pub channel_id: String,
}

#[derive(Serialize)]
pub struct WebrtcResponse {
    pub sdp_type: String,
    pub response: String,
    pub session_id: String,
    pub channel_id: String,
}

pub struct WebrtcStrategy {
    app_state: Arc<AppState>
}
impl RedisStrategy for WebrtcStrategy {
    type Message = WebrtcRequest;

    fn new(app_state: Arc<AppState>) -> Self {
        Self { app_state }
    }

    async fn handle_message(&self, message: Self::Message) {
        let channel = format!("{}:{}", WEBRTC_CHANNEL, message.session_id);
        let r = self.app_state.web_rtc_manager.lock().await.create_response_to_offer(message.sdp.clone(), message.session_id.clone(), message.channel_id.clone()).await;

        println!("Sending response to offer");
        if r.is_ok() {
            let response = WebrtcResponse {
                sdp_type: message.sdp_type.clone(),
                response: r.unwrap(),
                session_id: message.session_id,
                channel_id: message.channel_id,
            };

            let _ = self.app_state.redis_manager.publish(&channel, &response).await;
            println!("Sent response to offer");
        }

    }
}