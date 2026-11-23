//! What chat needs from the outside world.

use async_trait::async_trait;
use melior_domain::{
    AttachmentId, ChannelId, ChatContent, ChatMessage, ImageFormat, MessageId, StoredImage,
};
use std::time::Duration;

#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
#[error("chat store: {0}")]
pub struct ChatStoreError(pub String);

/// Durable storage for the messages of every room, and the record of every
/// uploaded image.
#[async_trait]
pub trait ChatRepository: Send + Sync {
    /// Saves a message. The store assigns its id and time, so ids order the
    /// messages of a room however many instances save to it. The images the
    /// message carries are marked attached in the same step, and a record that
    /// is gone by then fails the save.
    async fn append(
        &self,
        channel_id: &ChannelId,
        author: &str,
        content: &ChatContent,
    ) -> Result<ChatMessage, ChatStoreError>;

    /// Up to `limit` messages of a room, newest first. With `before`, only those
    /// with a lower id, which pages back through the history.
    async fn page(
        &self,
        channel_id: &ChannelId,
        before: Option<MessageId>,
        limit: usize,
    ) -> Result<Vec<ChatMessage>, ChatStoreError>;

    /// Records an uploaded image, once its bytes are stored. It is not attached
    /// until a message carries it.
    async fn save_image(&self, image: &StoredImage) -> Result<(), ChatStoreError>;

    /// Deletes the records of up to `limit` images that no message carries and
    /// that were uploaded more than `older_than` ago, by the store's clock, and
    /// returns their ids. One step: an image a message attaches at the same
    /// moment is either attached and kept, or gone and refused to the message.
    async fn delete_unattached_images(
        &self,
        older_than: Duration,
        limit: usize,
    ) -> Result<Vec<AttachmentId>, ChatStoreError>;

    /// The record of an uploaded image, if there is one.
    async fn image(&self, id: &AttachmentId) -> Result<Option<StoredImage>, ChatStoreError>;

    /// The records of those of `ids` that exist, in no particular order.
    async fn images(&self, ids: &[AttachmentId]) -> Result<Vec<StoredImage>, ChatStoreError>;
}

#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum ImageStoreError {
    /// This server has no image storage: uploads are off.
    #[error("image storage is not configured")]
    Unconfigured,

    #[error("image storage: {0}")]
    Unavailable(String),
}

/// Where the bytes of uploaded images live, under their ids.
#[async_trait]
pub trait ImageStore: Send + Sync {
    async fn put(
        &self,
        id: &AttachmentId,
        format: ImageFormat,
        bytes: Vec<u8>,
    ) -> Result<(), ImageStoreError>;

    /// The bytes of an image, or `None` when nothing is stored under the id.
    async fn get(&self, id: &AttachmentId) -> Result<Option<Vec<u8>>, ImageStoreError>;

    /// Removes an image's bytes. Removing what is not there succeeds.
    async fn delete(&self, id: &AttachmentId) -> Result<(), ImageStoreError>;
}
