use fred::types::Message;
use serde::Deserialize;
use crate::structs::channel_constants::CHANNEL_CONNECTION_CHANNEL;
use crate::structs::sfu_strategy::SFUStrategy;

pub struct HandleMessage<T: RedisStrategy> {
    strategy: T,
}

pub trait RedisStrategy {
    // 1. Define the type this strategy expects
    type Message: for<'a> Deserialize<'a>;

    fn handle_message(&self, message: Self::Message);

    fn check_format(&self, message: Message) {
        if let Some(payload) = message.value.as_string() {
            match serde_json::from_str::<Self::Message>(&payload) {
                Ok(parsed) => self.handle_message(parsed),
                Err(_) => eprintln!("[Redis] Schema mismatch on {}", message.channel),
            }
        }
    }
}

impl<T: RedisStrategy> HandleMessage<T> {
    pub fn new(strategy: T) -> Self {
        Self { strategy }
    }

    pub fn process(&self, message: Message) {
        // Use the strategy's checking logic
        self.strategy.check_format(message);
    }
}

pub struct ChannelHandler;
impl ChannelHandler  {
    pub fn handle(message: Message) {
        // Use a constants check to dispatch to the correct strategy

        if message.channel == CHANNEL_CONNECTION_CHANNEL {
            // SFUStrategy must implement RedisStrategy with Self::Message = ChannelMessage
            let handler = HandleMessage::new(SFUStrategy);
            handler.process(message);
        } else {
            eprintln!("[Redis] Unknown channel: {}", message.channel);
        }
    }
}