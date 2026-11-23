//! The chat model: what a message is, what it may carry, what may be said and how fast.

mod attachment;
mod error;
mod limit;
mod message;

pub use attachment::{
    Attachment, AttachmentId, AttachmentRequest, GifLink, ImageFormat, StoredImage,
};
pub use error::ChatError;
pub use limit::SendLimit;
pub use message::{ChatContent, ChatMessage, ChatText, MessageId};
