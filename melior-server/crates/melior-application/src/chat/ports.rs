//! What chat needs from the outside world.

use async_trait::async_trait;
use melior_domain::{ChannelId, ChatMessage, ChatText, MessageId};

#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
#[error("chat store: {0}")]
pub struct ChatStoreError(pub String);

/// Durable storage for the messages of every room.
#[async_trait]
pub trait ChatRepository: Send + Sync {
    /// Saves a message. The store assigns its id and time, so ids order the
    /// messages of a room however many instances save to it.
    async fn append(
        &self,
        channel_id: &ChannelId,
        author: &str,
        text: &ChatText,
    ) -> Result<ChatMessage, ChatStoreError>;

    /// Up to `limit` messages of a room, newest first. With `before`, only those
    /// with a lower id, which pages back through the history.
    async fn page(
        &self,
        channel_id: &ChannelId,
        before: Option<MessageId>,
        limit: usize,
    ) -> Result<Vec<ChatMessage>, ChatStoreError>;
}
