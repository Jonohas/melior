use super::ports::{ChatRepository, ChatStoreError};
use async_trait::async_trait;
use melior_domain::{ChannelId, ChatMessage, ChatText, MessageId};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

/// A [`ChatRepository`] that lives and dies with the process. For tests and for
/// running a single instance without a database; messages do not survive a restart.
#[derive(Default)]
pub struct InMemoryChatRepository {
    messages: Mutex<Vec<ChatMessage>>,
}

impl InMemoryChatRepository {
    fn lock(&self) -> MutexGuard<'_, Vec<ChatMessage>> {
        self.messages.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[async_trait]
impl ChatRepository for InMemoryChatRepository {
    async fn append(
        &self,
        channel_id: &ChannelId,
        author: &str,
        text: &ChatText,
    ) -> Result<ChatMessage, ChatStoreError> {
        let mut messages = self.lock();
        let next = messages.last().map_or(1, |last| last.id.value() + 1);
        let sent_at_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX));
        let message = ChatMessage {
            id: MessageId::new(next),
            channel_id: channel_id.clone(),
            author: author.to_owned(),
            text: text.clone(),
            sent_at_ms,
        };
        messages.push(message.clone());
        Ok(message)
    }

    async fn page(
        &self,
        channel_id: &ChannelId,
        before: Option<MessageId>,
        limit: usize,
    ) -> Result<Vec<ChatMessage>, ChatStoreError> {
        Ok(self
            .lock()
            .iter()
            .rev()
            .filter(|message| &message.channel_id == channel_id)
            .filter(|message| before.is_none_or(|before| message.id < before))
            .take(limit)
            .cloned()
            .collect())
    }
}
