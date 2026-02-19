use fred::types::Message;
use crate::shared::redis_strategy::RedisStrategy;

pub struct HandleMessage<T: RedisStrategy> {
    strategy: T,
}
impl<T: RedisStrategy> HandleMessage<T> {
    pub fn new(strategy: T) -> Self {
        Self { strategy }
    }

    pub async fn process(&self, message: Message) {
        // Use the strategy's checking logic
        self.strategy.check_format(message).await;
    }
}