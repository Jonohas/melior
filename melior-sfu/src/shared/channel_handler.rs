use std::sync::Arc;
use fred::types::Message;
use crate::shared::channel_constants::{CHANNEL_CONNECTION_CHANNEL, WEBRTC_CHANNEL};
use crate::shared::generic_message_handler::HandleMessage;
use crate::shared::redis_manager::RedisManager;
use crate::shared::redis_strategy::{RedisStrategy};
use crate::strategies::sfu_strategy::SFUStrategy;
use crate::strategies::webrtc_strategy::WebrtcStrategy;
use crate::structs::app_state::AppState;

pub struct ChannelHandler;
impl ChannelHandler  {
    pub async fn handle(message: Message, app_state: Arc<AppState>) {
        // Use a constants check to dispatch to the correct strategy

        app_state.redis_manager.hello();

        if message.channel == CHANNEL_CONNECTION_CHANNEL {
            // SFUStrategy must implement RedisStrategy with Self::Message redis_manager.clone()= ChannelMessage
            let handler = HandleMessage::new(SFUStrategy::new(app_state.clone()));
            handler.process(message).await;
        } else if message.channel == WEBRTC_CHANNEL {
            let handler = HandleMessage::new(WebrtcStrategy::new(app_state.clone()));
            handler.process(message).await;
        } else {
            eprintln!("[Redis] Unknown channel: {}", message.channel);
        }
    }
}