//! Delivery mechanisms that drive the application's use cases.
//!
//! - [`http`]: the login round trip, identity, health, room and chat history endpoints.
//! - [`ws`]: the signalling WebSocket.
//!
//! Invariants of this crate:
//!
//! - Handlers only translate: they turn a request into a use case call and its
//!   result into a response. They decide nothing.
//! - Wire formats are private DTOs, converted at the edge, so they can change
//!   without touching the application.
//! - It does not depend on `melior-infrastructure` or on any adapter's
//!   library (`cargo xtask layers` checks this).
//! - No request can make a handler panic.

#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

pub mod http;
pub mod ws;

use axum::Router;
use melior_application::{AuthService, ChatService, ConferenceService, RoomAccessService};
use std::sync::Arc;
use std::time::Duration;

/// What the request handlers need.
#[derive(Clone)]
pub struct AppState {
    pub conference: Arc<ConferenceService>,
    pub chat: Arc<ChatService>,
    pub auth: Arc<AuthService>,
    pub rooms: Arc<RoomAccessService>,
    pub session_cookie_name: Arc<str>,
    /// How often an open connection re-checks that it is still welcome.
    pub session_recheck: Duration,
}

/// The whole HTTP surface:
///
/// - `GET /login`, `/signin`, `/signout`: the login round trip (see [`http`]).
/// - `GET /api/identity/me`, `/api/health`.
/// - `GET /api/rooms`, `GET`/`PUT`/`DELETE /api/rooms/{channel_id}`: the room registry.
/// - `GET /api/rooms/{channel_id}/messages`: the room's chat history.
/// - `GET /ws?channel_id=<room>`: conference signalling (see [`ws`]), for logged-in users.
pub fn router(state: AppState) -> Router {
    Router::new().merge(http::routes()).merge(ws::routes()).with_state(state)
}
