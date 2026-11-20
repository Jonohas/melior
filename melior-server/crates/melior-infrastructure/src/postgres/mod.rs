//! Postgres adapters: the room registry and the chat history.

mod chat;
mod database;
mod rooms;

pub use chat::PostgresChatRepository;
pub use database::Database;
pub use rooms::PostgresRoomRepository;
