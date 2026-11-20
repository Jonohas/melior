//! `/api/channels/...`: one channel, its voice admission probe, and its messages.

use super::community_dto::ChannelViewDto;
use super::community_input::EditChannelDto;
use super::support::{Failure, body, caller, changing_caller, channel_id, done, reply};
use crate::AppState;
use crate::ws::chat::ChatMessageDto;
use axum::Router;
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::get;
use axum_extra::extract::cookie::CookieJar;
use melior_domain::MessageId;
use serde::{Deserialize, Serialize};

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/channels/{channel_id}", get(show).patch(edit).delete(remove))
        .route("/api/channels/{channel_id}/admission", get(admission))
        .route("/api/channels/{channel_id}/messages", get(history).post(post))
}

type Id = Path<String>;

/// `GET /api/channels/{channel_id}`: the channel view, for a member who may view it.
async fn show(State(state): State<AppState>, Path(id): Id, jar: CookieJar) -> Response {
    let (user, jar) = match caller(&state, jar).await {
        Ok(caller) => caller,
        Err(response) => return *response,
    };
    let result = async {
        let id = channel_id(&id)?;
        Ok::<_, Failure>(ChannelViewDto::from(state.communities.channel(&id, &user).await?))
    };
    reply(jar, result.await)
}

/// `PATCH /api/channels/{channel_id}`: `manage_channels`.
async fn edit(
    State(state): State<AppState>,
    Path(id): Id,
    headers: HeaderMap,
    jar: CookieJar,
    bytes: Bytes,
) -> Response {
    let (user, jar) = match changing_caller(&state, &headers, jar).await {
        Ok(caller) => caller,
        Err(response) => return *response,
    };
    let result = async {
        let id = channel_id(&id)?;
        let changes = body::<EditChannelDto>(&bytes)?.into_changes()?;
        let edited = state.communities.edit_channel(&id, &user, changes).await?;
        Ok::<_, Failure>(ChannelViewDto::from(edited))
    };
    reply(jar, result.await)
}

/// `DELETE /api/channels/{channel_id}`: `manage_channels`; deletes its history too.
async fn remove(
    State(state): State<AppState>,
    Path(id): Id,
    headers: HeaderMap,
    jar: CookieJar,
) -> Response {
    let (user, jar) = match changing_caller(&state, &headers, jar).await {
        Ok(caller) => caller,
        Err(response) => return *response,
    };
    let result = async {
        let id = channel_id(&id)?;
        Ok::<_, Failure>(state.communities.delete_channel(&id, &user).await?)
    };
    done(jar, result.await)
}

/// `GET /api/channels/{channel_id}/admission`: whether the caller may join the
/// voice channel now. 204 yes; 403, 404 or 409 (full) as the WebSocket upgrade
/// would answer, which a browser cannot see.
async fn admission(State(state): State<AppState>, Path(id): Id, jar: CookieJar) -> Response {
    let (user, jar) = match caller(&state, jar).await {
        Ok(caller) => caller,
        Err(response) => return *response,
    };
    let result = async {
        let id = channel_id(&id)?;
        Ok::<_, Failure>(state.communities.probe_voice(&id, &user).await?)
    };
    done(jar, result.await)
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

/// `GET /api/channels/{channel_id}/messages?before=<id>&limit=<n>`: a page of the
/// channel's history, newest page first. Needs `view_channel` and `read_history`.
async fn history(
    State(state): State<AppState>,
    Path(id): Id,
    Query(params): Query<HistoryParams>,
    jar: CookieJar,
) -> Response {
    let (user, jar) = match caller(&state, jar).await {
        Ok(caller) => caller,
        Err(response) => return *response,
    };
    let result = async {
        let id = channel_id(&id)?;
        let before = params.before.map(MessageId::new);
        let page = state.chat.history(&id, &user, before, params.limit).await?;
        Ok::<_, Failure>(HistoryDto {
            messages: page.messages.into_iter().map(|m| ChatMessageDto::new(m, None)).collect(),
            has_more: page.has_more,
        })
    };
    reply(jar, result.await)
}

/// `{ "text": ..., "client_id"?: ... }`.
#[derive(Deserialize)]
struct PostDto {
    text: String,
    #[serde(default)]
    client_id: Option<String>,
}

/// `POST /api/channels/{channel_id}/messages`: a message in a text channel (409 for
/// a voice channel). Needs `view_channel` and `send_messages`. Returns the saved
/// message with the caller's `client_id`; everyone else gets it on the feed.
async fn post(
    State(state): State<AppState>,
    Path(id): Id,
    headers: HeaderMap,
    jar: CookieJar,
    bytes: Bytes,
) -> Response {
    let (user, jar) = match changing_caller(&state, &headers, jar).await {
        Ok(caller) => caller,
        Err(response) => return *response,
    };
    let result = async {
        let id = channel_id(&id)?;
        let request: PostDto = body(&bytes)?;
        let message = state.chat.post(&id, &user, &request.text).await?;
        Ok::<_, Failure>(ChatMessageDto::new(message, request.client_id))
    };
    reply(jar, result.await)
}
