use super::session::{
    Authenticated, LOGIN_COMPLETED_COOKIE, LOGIN_COMPLETED_TTL, LOGIN_STATE_COOKIE,
    LOGIN_STATE_TTL, authenticate, cookie, forget, session_cookie,
};
use crate::AppState;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::http::header::{CACHE_CONTROL, LOCATION};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use axum_extra::extract::cookie::CookieJar;
use melior_domain::AuthSessionId;
use serde::{Deserialize, Serialize};
use tracing::warn;

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/login", get(login))
        .route("/signin", get(signin))
        .route("/signout", get(signout))
        .route("/api/identity/me", get(me))
        .route("/api/health", get(health))
}

fn found(location: &str) -> Response {
    (StatusCode::FOUND, [(LOCATION, location), (CACHE_CONTROL, "no-store")]).into_response()
}

#[derive(Deserialize)]
struct LoginParams {
    #[serde(rename = "redirectTo")]
    redirect_to: Option<String>,
}

/// `GET /login?redirectTo=<path>`: sends the browser to the identity provider.
async fn login(
    State(state): State<AppState>,
    Query(params): Query<LoginParams>,
    jar: CookieJar,
) -> Response {
    match state.auth.begin_login(params.redirect_to.as_deref()).await {
        Ok(redirect) => {
            let jar = jar.add(cookie(
                &state,
                LOGIN_STATE_COOKIE.to_owned(),
                redirect.login_state,
                LOGIN_STATE_TTL,
                true,
            ));
            (jar, found(&redirect.location)).into_response()
        }
        Err(error) => {
            warn!(%error, "could not start login");
            found(&state.auth.settings().frontend("/error?reason=idp-unavailable"))
        }
    }
}

#[derive(Deserialize)]
struct SigninParams {
    code: Option<String>,
    state: Option<String>,
}

/// `GET /signin?code=&state=`: the identity provider's callback. Opens the session
/// and returns the browser to where it wanted to go.
async fn signin(
    State(state): State<AppState>,
    Query(params): Query<SigninParams>,
    jar: CookieJar,
) -> Response {
    let settings = state.auth.settings();
    let Some(code) = params.code.as_deref().filter(|code| !code.trim().is_empty()) else {
        return found(&settings.frontend("/login"));
    };

    // The login state is single use, whatever happens next.
    let login_state = jar.get(LOGIN_STATE_COOKIE).map(|cookie| cookie.value().to_owned());
    let jar = forget(jar, LOGIN_STATE_COOKIE);

    match state.auth.complete_login(code, params.state.as_deref(), login_state.as_deref()).await {
        Ok(completed) => {
            let jar = jar.add(session_cookie(&state, &completed.session_id)).add(cookie(
                &state,
                LOGIN_COMPLETED_COOKIE.to_owned(),
                "1".to_owned(),
                LOGIN_COMPLETED_TTL,
                false,
            ));
            (jar, found(&completed.location)).into_response()
        }
        Err(error) => {
            warn!(%error, "login failed");
            (jar, found(&settings.frontend("/error?reason=login-failed"))).into_response()
        }
    }
}

/// `GET /signout`: ends the session here and at the identity provider.
async fn signout(State(state): State<AppState>, jar: CookieJar) -> Response {
    let name: &str = &state.session_cookie_name;
    let id = jar.get(name).and_then(|cookie| AuthSessionId::parse(cookie.value()));
    let location = state.auth.logout(id.as_ref()).await;
    (forget(jar, name), found(&location)).into_response()
}

#[derive(Serialize)]
struct IdentityResponse {
    subject: String,
    username: String,
    email: Option<String>,
    roles: Vec<String>,
}

/// `GET /api/identity/me`: who the session belongs to. 401 without a session, 403
/// for a user who holds no Melior role.
async fn me(State(state): State<AppState>, jar: CookieJar) -> Response {
    match authenticate(&state, jar).await {
        Ok(Authenticated { user, jar, .. }) if user.may_join_rooms() => (
            jar,
            [(CACHE_CONTROL, "no-store")],
            Json(IdentityResponse {
                subject: user.id.to_string(),
                username: user.username.clone(),
                email: user.email.clone(),
                roles: user.roles().to_vec(),
            }),
        )
            .into_response(),
        Ok(Authenticated { jar, .. }) => (jar, StatusCode::FORBIDDEN).into_response(),
        Err((rejection, jar)) => (jar, rejection.status()).into_response(),
    }
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
}

/// `GET /api/health`: liveness, no session needed.
async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}
