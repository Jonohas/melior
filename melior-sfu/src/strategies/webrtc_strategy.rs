use std::sync::Arc;
use serde::{Deserialize, Serialize};
use crate::redis_manager::RedisManager;
use crate::shared::redis_strategy::RedisStrategy;

#[derive(Debug, Serialize, Deserialize)]
pub struct WebrtcRequest {
    pub sdo_type: String,
    pub sdp: String,
    pub session_id: String,
    pub channel_id: String,
}

#[derive(Serialize)]
pub struct WebrtcResponse {
    pub sdp_type: String,
    pub sdp: String,
    pub session_id: String,
    pub channel_id: String,
}

pub struct WebrtcStrategy;
impl RedisStrategy for WebrtcStrategy {
    type Message = WebrtcRequest;

    fn handle_message(&self, message: Self::Message, _redis_manager: Arc<RedisManager>) {
        // Success! Fields are accessible here
        println!("Joining: {} {} {} {}", message.sdo_type, message.sdp, message.session_id, message.channel_id);
    }
}