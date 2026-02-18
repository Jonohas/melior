use serde::{Deserialize, Serialize};
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
    pub sdo_type: String,
    pub sdp: String,
    pub session_id: String,
    pub channel_id: String,
}

pub struct WebrtcStrategy;
impl RedisStrategy for WebrtcStrategy {
    type Message = WebrtcRequest; // The struct you defined earlier

    fn handle_message(&self, message: Self::Message) {
        // Success! Fields are accessible here
        // println!("Joining: {}", message);
    }
}