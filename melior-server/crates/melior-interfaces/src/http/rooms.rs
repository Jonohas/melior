//! The room registry API: who owns a room and who may enter it.

use super::session::{Authenticated, authenticate, from_foreign_origin};
use crate::AppState;
use axum::extract::{Path, State};
use axum::http::header::CACHE_CONTROL;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use axum_extra::extract::cookie::CookieJar;
use melior_application::RoomsError;
use melior_domain::{ChannelId, RoomAccess, User, Username};
use serde::{Deserialize, Serialize};
use tracing::warn;

/// An invitation list longer than this is refused rather than stored.
const MAX_MEMBERS: usize = 200;

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/rooms", get(list))
        .route("/api/rooms/{channel_id}", get(show).put(configure).delete(remove))
}

#[derive(Serialize)]
struct RoomDto {
    channel_id: String,
    /// The owner's username.
    owner: String,
    restricted: bool,
    /// Invited usernames, lower-case, sorted.
    members: Vec<String>,
    /// Whether the caller may change or delete the room (its owner, or an admin).
    is_owner: bool,
}

impl RoomDto {
    fn new(room: &RoomAccess, caller: &User) -> Self {
        Self {
            channel_id: room.channel_id().to_string(),
            owner: room.owner_username().to_owned(),
            restricted: room.is_restricted(),
            members: room.members().map(Username::to_string).collect(),
            is_owner: room.may_manage(caller),
        }
    }
}

#[derive(Deserialize)]
struct ConfigureRequest {
    restricted: bool,
    #[serde(default)]
    members: Vec<String>,
}

/// `GET /api/rooms`: the rooms the caller owns or is invited to; all of them for an admin.
async fn list(State(state): State<AppState>, jar: CookieJar) -> Response {
    let Authenticated { user, jar, .. } = match authenticate(&state, jar).await {
        Ok(authenticated) => authenticated,
        Err((rejection, jar)) => return (jar, rejection.status()).into_response(),
    };
    match state.rooms.list(&user).await {
        Ok(rooms) => {
            let rooms: Vec<RoomDto> = rooms.iter().map(|room| RoomDto::new(room, &user)).collect();
            (jar, [(CACHE_CONTROL, "no-store")], Json(rooms)).into_response()
        }
        Err(error) => (jar, rejected(&error)).into_response(),
    }
}

/// `GET /api/rooms/{channel_id}`: 404 for a channel nobody registered, 403 for a
/// restricted room the caller may not enter.
async fn show(
    State(state): State<AppState>,
    Path(channel_id): Path<String>,
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
    match state.rooms.get(&channel_id, &user).await {
        Ok(room) => {
            (jar, [(CACHE_CONTROL, "no-store")], Json(RoomDto::new(&room, &user))).into_response()
        }
        Err(error) => (jar, rejected(&error)).into_response(),
    }
}

/// `PUT /api/rooms/{channel_id}`: sets who may enter. Registers the room for the
/// caller if it does not exist; otherwise only its owner or an admin may do this.
async fn configure(
    State(state): State<AppState>,
    Path(channel_id): Path<String>,
    headers: HeaderMap,
    jar: CookieJar,
    Json(request): Json<ConfigureRequest>,
) -> Response {
    if from_foreign_origin(&state, &headers) {
        return (StatusCode::FORBIDDEN, "origin not allowed").into_response();
    }
    let Authenticated { user, jar, .. } = match authenticate(&state, jar).await {
        Ok(authenticated) => authenticated,
        Err((rejection, jar)) => return (jar, rejection.status()).into_response(),
    };
    let channel_id = match ChannelId::parse(&channel_id) {
        Ok(channel_id) => channel_id,
        Err(error) => return (jar, (StatusCode::BAD_REQUEST, error.to_string())).into_response(),
    };
    if request.members.len() > MAX_MEMBERS {
        let message = format!("at most {MAX_MEMBERS} members");
        return (jar, (StatusCode::BAD_REQUEST, message)).into_response();
    }
    match state.rooms.configure(&channel_id, &user, request.restricted, &request.members).await {
        Ok(room) => (jar, Json(RoomDto::new(&room, &user))).into_response(),
        Err(error) => (jar, rejected(&error)).into_response(),
    }
}

/// `DELETE /api/rooms/{channel_id}`: owner or admin only. The channel is
/// unregistered afterwards; whoever joins it next owns it.
async fn remove(
    State(state): State<AppState>,
    Path(channel_id): Path<String>,
    headers: HeaderMap,
    jar: CookieJar,
) -> Response {
    if from_foreign_origin(&state, &headers) {
        return (StatusCode::FORBIDDEN, "origin not allowed").into_response();
    }
    let Authenticated { user, jar, .. } = match authenticate(&state, jar).await {
        Ok(authenticated) => authenticated,
        Err((rejection, jar)) => return (jar, rejection.status()).into_response(),
    };
    let channel_id = match ChannelId::parse(&channel_id) {
        Ok(channel_id) => channel_id,
        Err(error) => return (jar, (StatusCode::BAD_REQUEST, error.to_string())).into_response(),
    };
    match state.rooms.delete(&channel_id, &user).await {
        Ok(()) => (jar, StatusCode::NO_CONTENT).into_response(),
        Err(error) => (jar, rejected(&error)).into_response(),
    }
}

/// The response for a refused or failed room request. Storage errors are logged
/// here and not shown: they can carry connection details.
pub(super) fn rejected(error: &RoomsError) -> Response {
    match error {
        RoomsError::NotFound => StatusCode::NOT_FOUND.into_response(),
        RoomsError::Forbidden => StatusCode::FORBIDDEN.into_response(),
        RoomsError::Domain(error) => (StatusCode::BAD_REQUEST, error.to_string()).into_response(),
        RoomsError::Repository(error) => {
            warn!(%error, "room registry unavailable");
            StatusCode::SERVICE_UNAVAILABLE.into_response()
        }
    }
}
