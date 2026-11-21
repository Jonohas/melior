//! The WebSockets: conference signalling in a voice channel, and a community's feed.

pub mod chat;
mod feed;
pub mod feed_messages;
mod handler;
mod limiter;
pub mod messages;
pub mod peer;
mod writer;

use crate::AppState;
use axum::Router;

/// `GET /ws?channel_id=<id>` and `GET /ws/communities/{id}`.
pub fn routes() -> Router<AppState> {
    handler::routes().merge(feed::routes())
}
