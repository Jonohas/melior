use fred::types::Message;
use serde::{Deserialize, Serialize};
use crate::structs::redis_strategy::RedisStrategy;

#[derive(Debug, Serialize, Deserialize)]
pub struct ChannelMessage {
    pub channel_id: String,
    pub session_id: String
}

pub struct SFUStrategy;
impl RedisStrategy for SFUStrategy {
    type Message = ChannelMessage; // The struct you defined earlier

    fn handle_message(&self, message: Self::Message) {
        // Success! Fields are accessible here
        println!("Joining: {}", message.channel_id);
    }
}