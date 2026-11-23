//! Room chat: sending a message to the people in a room, reading what was said,
//! and the images messages carry.

mod images;
mod limits;
mod memory;
mod ports;
mod service;
mod sweeper;

pub use images::ImageFile;
pub use memory::{InMemoryChatRepository, InMemoryImageStore, UnconfiguredImageStore};
pub use ports::{ChatRepository, ChatStoreError, ImageStore, ImageStoreError};
pub use service::{ChatDraft, ChatFailure, ChatPage, ChatService};
pub use sweeper::start_image_sweeper;
