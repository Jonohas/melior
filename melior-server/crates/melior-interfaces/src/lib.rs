//! Delivery mechanisms that drive the application's use cases.
//!
//! - [`http`]: the login round trip, identity, health and the community API.
//! - [`ws`]: the signalling WebSocket and the community feed.
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
use melior_application::{AuthService, ChatService, CommunityService, ConferenceService};
use std::sync::Arc;
use std::time::Duration;

/// What the request handlers need.
#[derive(Clone)]
pub struct AppState {
    pub conference: Arc<ConferenceService>,
    pub chat: Arc<ChatService>,
    pub auth: Arc<AuthService>,
    pub communities: Arc<CommunityService>,
    pub session_cookie_name: Arc<str>,
    /// How often an open connection re-checks that it is still welcome.
    pub session_recheck: Duration,
    /// The KLIPY app key browsers search GIFs with; `None` turns GIFs off.
    pub klipy_app_key: Option<Arc<str>>,
}

/// The whole HTTP surface:
///
/// - `GET /login`, `/signin`, `/signout`: the login round trip (see [`http`]).
/// - `GET /api/identity/me`, `/api/health`.
/// - `/api/communities/...`, `/api/channels/...`, `/api/invites/{code}`: the community API.
/// - `/api/attachments/{id}`, `/api/gifs/config`: chat images and the GIF search settings.
/// - `GET /ws?channel_id=<id>`: conference signalling in a voice channel (see [`ws`]).
/// - `GET /ws/communities/{id}`: a community's feed of voice presence and messages.
pub fn router(state: AppState) -> Router {
    Router::new().merge(http::routes()).merge(ws::routes()).with_state(state)
}
