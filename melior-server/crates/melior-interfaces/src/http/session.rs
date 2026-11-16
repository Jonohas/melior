//! Session cookie handling shared by the HTTP and WebSocket handlers.

use crate::AppState;
use axum::http::header::ORIGIN;
use axum::http::{HeaderMap, StatusCode};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use melior_application::SessionState;
use melior_domain::{AuthSessionId, User};
use std::time::Duration;
use tracing::warn;

/// Holds the PKCE verifier and nonce between `/login` and `/signin`.
pub const LOGIN_STATE_COOKIE: &str = "bff_pkce";
/// Readable by the frontend for a few seconds after a login completed, so it can
/// tell "the login did not stick" from "the session expired" and not loop.
pub const LOGIN_COMPLETED_COOKIE: &str = "bff_login_completed";

pub(super) const LOGIN_STATE_TTL: Duration = Duration::from_secs(600);
pub(super) const LOGIN_COMPLETED_TTL: Duration = Duration::from_secs(30);

/// Why a request is not served.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rejection {
    /// No valid session.
    Unauthenticated,
    /// The session could not be checked right now.
    Unavailable,
}

impl Rejection {
    #[must_use]
    pub fn status(self) -> StatusCode {
        match self {
            Self::Unauthenticated => StatusCode::UNAUTHORIZED,
            Self::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        }
    }
}

/// A request that carries a valid session.
pub struct Authenticated {
    pub user: User,
    pub session_id: AuthSessionId,
    /// Cookie changes to send with the response: a renewed session cookie after a
    /// token refresh.
    pub jar: CookieJar,
}

/// Resolves the session cookie to a user. A rejection carries the jar too: it
/// removes the cookie when the session is gone, and leaves it alone when the
/// session merely could not be verified.
pub async fn authenticate(
    state: &AppState,
    jar: CookieJar,
) -> Result<Authenticated, (Rejection, CookieJar)> {
    let name: &str = &state.session_cookie_name;
    let Some(cookie) = jar.get(name) else {
        return Err((Rejection::Unauthenticated, jar));
    };
    let Some(session_id) = AuthSessionId::parse(cookie.value()) else {
        return Err((Rejection::Unauthenticated, forget(jar, name)));
    };

    match state.auth.check(&session_id).await {
        Ok(SessionState::Active(active)) => {
            let jar =
                if active.renewed { jar.add(session_cookie(state, &session_id)) } else { jar };
            Ok(Authenticated { user: active.user, session_id, jar })
        }
        Ok(SessionState::Ended) => Err((Rejection::Unauthenticated, forget(jar, name))),
        Ok(SessionState::Unverifiable) => Err((Rejection::Unavailable, jar)),
        Err(error) => {
            warn!(%error, "session check failed");
            Err((Rejection::Unavailable, jar))
        }
    }
}

/// True if a browser sent this request from a page of another site.
///
/// Browsers attach `Origin` to WebSocket handshakes and to requests that change
/// something, and they do not stop either from carrying the user's cookie, so
/// this check is what does. A request without `Origin` is not from a browser page.
pub fn from_foreign_origin(state: &AppState, headers: &HeaderMap) -> bool {
    headers
        .get(ORIGIN)
        .is_some_and(|origin| origin.as_bytes() != state.auth.settings().frontend_url.as_bytes())
}

pub(super) fn session_cookie(state: &AppState, id: &AuthSessionId) -> Cookie<'static> {
    cookie(
        state,
        state.session_cookie_name.to_string(),
        id.as_str().to_owned(),
        state.auth.settings().session_ttl,
        true,
    )
}

/// A host-only, `SameSite=Lax` cookie for the whole application. `Secure` whenever
/// the application is served over https.
pub(super) fn cookie(
    state: &AppState,
    name: String,
    value: String,
    max_age: Duration,
    http_only: bool,
) -> Cookie<'static> {
    Cookie::build((name, value))
        .path("/")
        .http_only(http_only)
        .same_site(SameSite::Lax)
        .secure(state.auth.settings().is_https())
        .max_age(time::Duration::seconds(i64::try_from(max_age.as_secs()).unwrap_or(i64::MAX)))
        .build()
}

pub(super) fn forget(jar: CookieJar, name: &str) -> CookieJar {
    jar.remove(Cookie::build(name.to_owned()).path("/"))
}
