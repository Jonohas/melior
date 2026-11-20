//! The chat history API.

use super::rooms::rejected;
use super::session::{Authenticated, authenticate};
use crate::AppState;
use crate::ws::messages::ChatMessageDto;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::http::header::CACHE_CONTROL;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use axum_extra::extract::cookie::CookieJar;
use melior_domain::{ChannelId, MessageId};
use serde::{Deserialize, Serialize};
use tracing::warn;

pub(super) fn routes() -> Router<AppState> {
    Router::new().route("/api/rooms/{channel_id}/messages", get(history))
}

#[derive(Deserialize)]
struct HistoryParams {
    /// Only messages with a lower id than this.
    before: Option<i64>,
    limit: Option<usize>,
}

#[derive(Serialize)]
struct HistoryDto {
    /// Oldest first.
    messages: Vec<ChatMessageDto>,
    /// Whether older messages exist than the oldest one here.
    has_more: bool,
}

/// `GET /api/rooms/{channel_id}/messages?before=<id>&limit=<n>`: a page of a room's
/// chat history, newest page first. The same people may read it as may enter the
/// room: 404 for a channel nobody registered, 403 for a room the caller may not enter.
async fn history(
    State(state): State<AppState>,
    Path(channel_id): Path<String>,
    Query(params): Query<HistoryParams>,
    jar: CookieJar,
) -> Response {
    let Authenticated { user, jar, .. } = match authenticate(&state, jar).await {
        Ok(authenticated) => authenticated,
        Err((rejection, jar)) => return (jar, rejection.status()).into_response(),
    };
    let channel_id = match ChannelId::parse(&channel_id) {
        Ok(channel_id) => channel_id,
        Err(error) => return (jar, (StatusCode::BAD_REQUEST, error.to_string())).into_response(),
    };
    if let Err(error) = state.rooms.get(&channel_id, &user).await {
        return (jar, rejected(&error)).into_response();
    }

    match state.chat.history(&channel_id, params.before.map(MessageId::new), params.limit).await {
        Ok(page) => {
            let body = HistoryDto {
                messages: page.messages.into_iter().map(|m| ChatMessageDto::new(m, None)).collect(),
                has_more: page.has_more,
            };
            (jar, [(CACHE_CONTROL, "no-store")], Json(body)).into_response()
        }
        Err(error) => {
            warn!(%error, channel = %channel_id, "chat history unavailable");
            (jar, StatusCode::SERVICE_UNAVAILABLE).into_response()
        }
    }
}
