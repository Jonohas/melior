//! Room chat: sending a message to the people in a room, and reading what was said.

mod memory;
mod ports;
mod service;

pub use memory::InMemoryChatRepository;
pub use ports::{ChatRepository, ChatStoreError};
pub use service::{ChatFailure, ChatPage, ChatService};
