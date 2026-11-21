//! Postgres adapters: communities (with their channels, roles and invites) and the
//! chat history.

mod chat;
mod communities;
mod database;

pub use chat::PostgresChatRepository;
pub use communities::PostgresCommunityRepository;
pub use database::Database;
