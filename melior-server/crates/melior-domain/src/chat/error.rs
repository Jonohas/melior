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
}
