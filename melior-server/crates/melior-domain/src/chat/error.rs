use crate::community::Permissions;

/// A violated chat rule. The wording reaches the person who sent the message.
#[derive(thiserror::Error, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatError {
    #[error("a message cannot be empty")]
    Empty,

    #[error("a message is at most 2000 characters")]
    TooLong,

    #[error("a message cannot contain control characters")]
    InvalidCharacter,

    #[error("you are sending messages too fast, wait a moment")]
    RateLimited,

    #[error("you may not send messages in this channel")]
    NotAllowed,

    #[error("an image must be a PNG, JPEG, GIF or WebP file")]
    UnsupportedImage,

    #[error("an image is at most 10 MB")]
    ImageTooLarge,

    #[error("the image was not found, upload it again")]
    UnknownImage,

    #[error("a GIF must come from KLIPY")]
    InvalidGif,

    #[error("a message carries at most 10 images")]
    TooManyImages,

    #[error("a GIF is sent on its own")]
    GifNotAlone,

    #[error("an image is attached twice")]
    DuplicateImage,
}

impl ChatError {
    /// Sending into a channel takes `send_messages` there.
    pub fn require_send(permissions: Permissions) -> Result<(), Self> {
        if permissions.contains(Permissions::SEND_MESSAGES) {
            return Ok(());
        }
        Err(Self::NotAllowed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sending_needs_send_messages() {
        assert_eq!(ChatError::require_send(Permissions::SEND_MESSAGES), Ok(()));
        assert_eq!(ChatError::require_send(Permissions::ALL), Ok(()));
        assert_eq!(
            ChatError::require_send(Permissions::VIEW_CHANNEL.union(Permissions::CONNECT)),
            Err(ChatError::NotAllowed)
        );
    }
}
