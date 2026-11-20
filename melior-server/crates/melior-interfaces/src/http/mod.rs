//! The backend-for-frontend endpoints: login round trip, identity, health, the room
//! registry and the chat history.

mod auth;
mod chat;
mod rooms;
mod session;

pub use session::{
    Authenticated, LOGIN_COMPLETED_COOKIE, LOGIN_STATE_COOKIE, Rejection, authenticate,
    from_foreign_origin,
};

use crate::AppState;
use axum::Router;

/// The login round trip, identity, health and the room registry.
pub fn routes() -> Router<AppState> {
    auth::routes().merge(rooms::routes()).merge(chat::routes())
}
