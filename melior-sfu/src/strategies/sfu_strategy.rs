use std::sync::Arc;
use serde::{Deserialize, Serialize};
use crate::redis_manager::RedisManager;
use crate::shared::redis_strategy::RedisStrategy;

#[derive(Debug, Serialize, Deserialize)]
pub struct ChannelMessage {
    pub channel_id: String,
    pub session_id: String
}

pub struct SFUStrategy;
impl RedisStrategy for SFUStrategy {
    type Message = ChannelMessage; // The struct you defined earlier

    fn handle_message(&self, message: Self::Message, _redis_manager: Arc<RedisManager>) {
        // Success! Fields are accessible here
        println!("Joining: {}", message.channel_id);
    }
}