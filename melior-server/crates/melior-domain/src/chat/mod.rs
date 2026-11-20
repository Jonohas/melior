//! The chat model: what a message is, what may be said and how fast.

mod error;
mod limit;
mod message;

pub use error::ChatError;
pub use limit::SendLimit;
pub use message::{ChatMessage, ChatText, MessageId};
