//! The room registry: who owns a room and who may enter it.

mod memory;
mod ports;
mod service;

pub use memory::InMemoryRoomRepository;
pub use ports::{RepositoryError, RoomRepository};
pub use service::{RoomAccessService, RoomsError};
