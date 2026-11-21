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
