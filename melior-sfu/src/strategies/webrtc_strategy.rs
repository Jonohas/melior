use std::sync::Arc;
use serde::{Deserialize, Serialize};
use crate::shared::channel_constants::WEBRTC_CHANNEL;
use crate::shared::redis_manager::RedisManager;
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
        let r = self.app_state.web_rtc_manager.create_response_to_offer(message.sdp.clone()).await.unwrap();

        let response = WebrtcResponse {
            sdp_type: message.sdp_type.clone(),
            response: r.clone(),
            session_id: message.session_id.clone(),
            channel_id: message.channel_id.clone(),
        };

        let _ = self.app_state.redis_manager.publish(&channel, &response).await;
        println!("Received WebRTC SDP offer");
    }
}