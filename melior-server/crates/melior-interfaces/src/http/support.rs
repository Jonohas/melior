//! What every JSON endpoint of the community API shares: who is calling, how a
//! body and an id are read, and how a failure becomes a response.

use super::session::{Authenticated, authenticate, from_foreign_origin};
use crate::AppState;
use axum::Json;
use axum::body::Bytes;
use axum::http::header::CACHE_CONTROL;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum_extra::extract::cookie::CookieJar;
use melior_application::{ChatFailure, CommunitiesError};
use melior_domain::{CategoryId, ChannelId, ChatError, CommunityId, InviteCode, RoleId, User};
use serde::Serialize;
use serde::de::DeserializeOwned;
use tracing::warn;

/// Why a request is not served, once the caller is known.
#[derive(Debug)]
pub(super) enum Failure {
    /// Invalid input: 400 with the reason as plain text.
    Bad(String),
    Community(CommunitiesError),
    Chat(ChatFailure),
}

impl From<CommunitiesError> for Failure {
    fn from(error: CommunitiesError) -> Self {
        Self::Community(error)
    }
}

impl From<ChatFailure> for Failure {
    fn from(error: ChatFailure) -> Self {
        Self::Chat(error)
    }
}

impl From<melior_domain::CommunityError> for Failure {
    fn from(error: melior_domain::CommunityError) -> Self {
        Self::Bad(error.to_string())
    }
}

impl IntoResponse for Failure {
    fn into_response(self) -> Response {
        match self {
            Self::Bad(reason) => (StatusCode::BAD_REQUEST, reason).into_response(),
            Self::Community(error) => community_response(&error),
            Self::Chat(error) => chat_response(&error),
        }
    }
}

/// The response for a refused or failed community request. Storage errors are
/// logged here and not shown: they can carry connection details.
pub(crate) fn community_response(error: &CommunitiesError) -> Response {
    match error {
        CommunitiesError::NotFound => StatusCode::NOT_FOUND.into_response(),
        CommunitiesError::Forbidden => StatusCode::FORBIDDEN.into_response(),
        CommunitiesError::Refused(refusal) => {
            (StatusCode::FORBIDDEN, refusal.to_string()).into_response()
        }
        CommunitiesError::Invalid(error) => {
            (StatusCode::BAD_REQUEST, error.to_string()).into_response()
        }
        CommunitiesError::WrongKind | CommunitiesError::Full => {
            (StatusCode::CONFLICT, error.to_string()).into_response()
        }
        CommunitiesError::Gone => (StatusCode::GONE, error.to_string()).into_response(),
        CommunitiesError::Store(error) => {
            warn!(%error, "community store unavailable");
            StatusCode::SERVICE_UNAVAILABLE.into_response()
        }
    }
}

fn chat_response(error: &ChatFailure) -> Response {
    match error {
        ChatFailure::Community(error) => community_response(error),
        ChatFailure::Rejected(ChatError::RateLimited) => {
            (StatusCode::TOO_MANY_REQUESTS, error.to_string()).into_response()
        }
        ChatFailure::Rejected(ChatError::NotAllowed) => {
            (StatusCode::FORBIDDEN, error.to_string()).into_response()
        }
        ChatFailure::Rejected(rule) => (StatusCode::BAD_REQUEST, rule.to_string()).into_response(),
        ChatFailure::Store(_) | ChatFailure::Conference(_) => {
            warn!(%error, "chat unavailable");
            StatusCode::SERVICE_UNAVAILABLE.into_response()
        }
    }
}

/// The logged-in caller, or the response that refuses the request (boxed: a
/// response is large, and the success path is the common one).
pub(super) async fn caller(
    state: &AppState,
    jar: CookieJar,
) -> Result<(User, CookieJar), Box<Response>> {
    match authenticate(state, jar).await {
        Ok(Authenticated { user, jar, .. }) => Ok((user, jar)),
        Err((rejection, jar)) => Err(Box::new((jar, rejection.status()).into_response())),
    }
}

/// Like [`caller`], for a request that changes something: it must also come from
/// the application's own origin.
pub(super) async fn changing_caller(
    state: &AppState,
    headers: &HeaderMap,
    jar: CookieJar,
) -> Result<(User, CookieJar), Box<Response>> {
    if from_foreign_origin(state, headers) {
        return Err(Box::new((StatusCode::FORBIDDEN, "origin not allowed").into_response()));
    }
    caller(state, jar).await
}

/// A JSON reply that is never cached, or the failure.
pub(super) fn reply<T: Serialize>(jar: CookieJar, result: Result<T, Failure>) -> Response {
    match result {
        Ok(body) => (jar, [(CACHE_CONTROL, "no-store")], Json(body)).into_response(),
        Err(failure) => (jar, failure).into_response(),
    }
}

/// 204 on success, or the failure.
pub(super) fn done(jar: CookieJar, result: Result<(), Failure>) -> Response {
    match result {
        Ok(()) => (jar, StatusCode::NO_CONTENT).into_response(),
        Err(failure) => (jar, failure).into_response(),
    }
}

/// A JSON body; anything that does not fit is a 400 saying why. No body at all
/// reads as `{}`, so a request whose fields are all optional may omit it.
pub(super) fn body<T: DeserializeOwned>(bytes: &Bytes) -> Result<T, Failure> {
    let json: &[u8] = if bytes.is_empty() { b"{}" } else { bytes };
    serde_json::from_slice(json).map_err(|error| Failure::Bad(format!("invalid body: {error}")))
}

pub(super) fn community_id(raw: &str) -> Result<CommunityId, Failure> {
    CommunityId::parse(raw).ok_or_else(|| Failure::Bad("invalid community id".into()))
}

pub(super) fn role_id(raw: &str) -> Result<RoleId, Failure> {
    RoleId::parse(raw).ok_or_else(|| Failure::Bad("invalid role id".into()))
}

pub(super) fn category_id(raw: &str) -> Result<CategoryId, Failure> {
    CategoryId::parse(raw).ok_or_else(|| Failure::Bad("invalid category id".into()))
}

pub(super) fn channel_id(raw: &str) -> Result<ChannelId, Failure> {
    ChannelId::parse(raw).map_err(|error| Failure::Bad(error.to_string()))
}

pub(super) fn invite_code(raw: &str) -> Result<InviteCode, Failure> {
    InviteCode::parse(raw).ok_or_else(|| Failure::Bad("invalid invite code".into()))
}
