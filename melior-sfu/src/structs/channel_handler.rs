use fred::types::Message;
use crate::shared::channel_constants::{CHANNEL_CONNECTION_CHANNEL, WEBRTC_CHANNEL};
use crate::shared::redis_strategy::HandleMessage;
use crate::strategies::sfu_strategy::SFUStrategy;
use crate::strategies::webrtc_strategy::WebrtcStrategy;

pub struct ChannelHandler;
impl ChannelHandler  {
    pub fn handle(message: Message) {
        // Use a constants check to dispatch to the correct strategy

        if message.channel == CHANNEL_CONNECTION_CHANNEL {
            // SFUStrategy must implement RedisStrategy with Self::Message = ChannelMessage
            let handler = HandleMessage::new(SFUStrategy);
            handler.process(message);
        } else if message.channel.starts_with(WEBRTC_CHANNEL) {
            let handler = HandleMessage::new(WebrtcStrategy);
            handler.process(message);
        } else {
            eprintln!("[Redis] Unknown channel: {}", message.channel);
        }
    }
}