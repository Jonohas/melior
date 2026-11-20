//! The backend-for-frontend endpoints: login round trip, identity, health, and the
//! community API (communities, channels, messages, invites).

mod auth;
mod channels;
mod communities;
mod community_dto;
mod community_input;
mod invites;
mod session;
mod support;

pub use session::{
    Authenticated, LOGIN_COMPLETED_COOKIE, LOGIN_STATE_COOKIE, Rejection, authenticate,
    from_foreign_origin,
};
pub(crate) use support::community_response;

use crate::AppState;
use axum::Router;

/// The login round trip, identity, health and the community API.
pub fn routes() -> Router<AppState> {
    auth::routes().merge(communities::routes()).merge(channels::routes()).merge(invites::routes())
}
