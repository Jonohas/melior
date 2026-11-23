use super::ports::{ChatRepository, ChatStoreError, ImageStore, ImageStoreError};
use async_trait::async_trait;
use melior_domain::{
    Attachment, AttachmentId, ChannelId, ChatContent, ChatMessage, ImageFormat, MessageId,
    StoredImage,
};
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// A [`ChatRepository`] that lives and dies with the process. For tests and for
/// running a single instance without a database; messages do not survive a restart.
#[derive(Default)]
pub struct InMemoryChatRepository {
    messages: Mutex<Vec<ChatMessage>>,
    images: Mutex<HashMap<AttachmentId, ImageEntry>>,
}

/// An image record with what decides its cleanup.
struct ImageEntry {
    image: StoredImage,
    uploaded_at: Instant,
    attached: bool,
}

impl InMemoryChatRepository {
    fn lock(&self) -> MutexGuard<'_, Vec<ChatMessage>> {
        self.messages.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn images(&self) -> MutexGuard<'_, HashMap<AttachmentId, ImageEntry>> {
        self.images.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[async_trait]
impl ChatRepository for InMemoryChatRepository {
    async fn append(
        &self,
        channel_id: &ChannelId,
        author: &str,
        content: &ChatContent,
    ) -> Result<ChatMessage, ChatStoreError> {
        if let Some(Attachment::Image { id, .. }) = content.attachment() {
            let mut images = self.images();
            let entry =
                images.get_mut(id).ok_or_else(|| ChatStoreError(format!("no image {id}")))?;
            entry.attached = true;
        }
        let mut messages = self.lock();
        let next = messages.last().map_or(1, |last| last.id.value() + 1);
        let sent_at_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX));
        let message = ChatMessage {
            id: MessageId::new(next),
            channel_id: channel_id.clone(),
            author: author.to_owned(),
            content: content.clone(),
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

    async fn save_image(&self, image: &StoredImage) -> Result<(), ChatStoreError> {
        let entry =
            ImageEntry { image: image.clone(), uploaded_at: Instant::now(), attached: false };
        self.images().insert(image.id.clone(), entry);
        Ok(())
    }

    async fn image(&self, id: &AttachmentId) -> Result<Option<StoredImage>, ChatStoreError> {
        Ok(self.images().get(id).map(|entry| entry.image.clone()))
    }

    async fn delete_unattached_images(
        &self,
        older_than: Duration,
        limit: usize,
    ) -> Result<Vec<AttachmentId>, ChatStoreError> {
        let mut images = self.images();
        let expired: Vec<AttachmentId> = images
            .values()
            .filter(|entry| !entry.attached && entry.uploaded_at.elapsed() >= older_than)
            .take(limit)
            .map(|entry| entry.image.id.clone())
            .collect();
        for id in &expired {
            images.remove(id);
        }
        Ok(expired)
    }
}

/// An [`ImageStore`] in memory, for tests.
#[derive(Default)]
pub struct InMemoryImageStore {
    blobs: Mutex<HashMap<AttachmentId, Vec<u8>>>,
}

#[async_trait]
impl ImageStore for InMemoryImageStore {
    async fn put(
        &self,
        id: &AttachmentId,
        _format: ImageFormat,
        bytes: Vec<u8>,
    ) -> Result<(), ImageStoreError> {
        self.blobs.lock().unwrap_or_else(PoisonError::into_inner).insert(id.clone(), bytes);
        Ok(())
    }

    async fn get(&self, id: &AttachmentId) -> Result<Option<Vec<u8>>, ImageStoreError> {
        Ok(self.blobs.lock().unwrap_or_else(PoisonError::into_inner).get(id).cloned())
    }

    async fn delete(&self, id: &AttachmentId) -> Result<(), ImageStoreError> {
        self.blobs.lock().unwrap_or_else(PoisonError::into_inner).remove(id);
        Ok(())
    }
}

/// The [`ImageStore`] of a server without image storage: every upload and
/// download is refused as [`ImageStoreError::Unconfigured`].
pub struct UnconfiguredImageStore;

#[async_trait]
impl ImageStore for UnconfiguredImageStore {
    async fn put(
        &self,
        _id: &AttachmentId,
        _format: ImageFormat,
        _bytes: Vec<u8>,
    ) -> Result<(), ImageStoreError> {
        Err(ImageStoreError::Unconfigured)
    }

    async fn get(&self, _id: &AttachmentId) -> Result<Option<Vec<u8>>, ImageStoreError> {
        Err(ImageStoreError::Unconfigured)
    }

    async fn delete(&self, _id: &AttachmentId) -> Result<(), ImageStoreError> {
        Err(ImageStoreError::Unconfigured)
    }
}
