use std::sync::Arc;
use serde::{Deserialize, Serialize};
use crate::shared::redis_strategy::RedisStrategy;
use crate::structs::app_state::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct ChannelMessage {
    pub channel_id: String,
    pub session_id: String
}

pub struct SFUStrategy {
}
impl RedisStrategy for SFUStrategy {
    type Message = ChannelMessage;

    fn new(_: Arc<AppState>) -> Self {
        Self {  }
    }

    async fn handle_message(&self, message: Self::Message) {
        // Success! Fields are accessible here
        println!("Joining: {}", message.channel_id);
    }
}