//! The images chat messages carry: uploading one for a message, and showing it
//! to those who may read the channel it was sent in.

use super::service::{ChatFailure, ChatService};
use crate::communities::CommunitiesError;
use melior_domain::{AttachmentId, ChannelId, ImageFormat, StoredImage, User};

/// An image and its bytes, for a reader of its channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageFile {
    pub image: StoredImage,
    pub bytes: Vec<u8>,
}

impl ChatService {
    /// Stores an image a member uploads to attach to a message in `channel_id`,
    /// text or voice: needs `view_channel` and `send_messages` there. The bytes
    /// decide the format, whatever the client called it.
    #[tracing::instrument(name = "chat.upload_image", skip_all, fields(channel = %channel_id, user = %user.username))]
    pub async fn upload_image(
        &self,
        channel_id: &ChannelId,
        user: &User,
        bytes: Vec<u8>,
    ) -> Result<StoredImage, ChatFailure> {
        self.communities.send_grant(channel_id, user).await?;
        let format = ImageFormat::check_upload(&bytes)?;
        self.upload_limits.record(&user.id)?;
        let image = StoredImage {
            id: AttachmentId::generate(),
            channel_id: channel_id.clone(),
            uploader: user.username.clone(),
            format,
            size: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        };
        // Bytes first: a record never names an image that is not there.
        self.images.put(&image.id, format, bytes).await?;
        self.messages.save_image(&image).await?;
        Ok(image)
    }

    /// An uploaded image, for a user who may read the history of the channel it
    /// was uploaded to (`view_channel` and `read_history`). Unknown is 404.
    pub async fn image(&self, id: &AttachmentId, user: &User) -> Result<ImageFile, ChatFailure> {
        let image = self.messages.image(id).await?.ok_or(CommunitiesError::NotFound)?;
        self.communities.history_grant(&image.channel_id, user).await?;
        let bytes = self.images.get(id).await?.ok_or(CommunitiesError::NotFound)?;
        Ok(ImageFile { image, bytes })
    }
}
