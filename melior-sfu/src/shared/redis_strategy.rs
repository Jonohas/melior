use std::sync::Arc;
use fred::types::Message;
use serde::{Deserialize};
use crate::structs::app_state::AppState;

pub trait RedisStrategy {
    // 1. Define the type this strategy expects
    type Message: for<'a> Deserialize<'a>;

    fn new(redis_manager: Arc<AppState>) -> Self;

    async fn handle_message(&self, message: Self::Message);

    async fn check_format(&self, message: Message) {
        if let Some(payload) = message.value.as_string() {
            match serde_json::from_str::<Self::Message>(&payload) {
                Ok(parsed) => self.handle_message(parsed).await,
                Err(_) => eprintln!("[Redis] Schema mismatch on {}", message.channel),
            }
        }
    }
}


