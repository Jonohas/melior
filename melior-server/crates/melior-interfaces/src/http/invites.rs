//! `/api/invites/{code}`: previewing, redeeming and deleting an invite.

use super::community_dto::{InvitePreviewDto, JoinedDto};
use super::support::{Failure, caller, changing_caller, done, invite_code, reply};
use crate::AppState;
use axum::Router;
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::get;
use axum_extra::extract::cookie::CookieJar;

pub(super) fn routes() -> Router<AppState> {
    Router::new().route("/api/invites/{code}", get(preview).post(redeem).delete(remove))
}

type Code = Path<String>;

/// `GET /api/invites/{code}`: where the invite leads, for anyone with a Melior role.
async fn preview(State(state): State<AppState>, Path(code): Code, jar: CookieJar) -> Response {
    let (user, jar) = match caller(&state, jar).await {
        Ok(caller) => caller,
        Err(response) => return *response,
    };
    let result = async {
        let code = invite_code(&code)?;
        let preview = state.communities.preview_invite(&code, &user).await?;
        Ok::<_, Failure>(InvitePreviewDto::from(preview))
    };
    reply(jar, result.await)
}

/// `POST /api/invites/{code}`: joins the community; 410 when the invite is used up
/// or expired. A member gets the community id again without using the invite.
async fn redeem(
    State(state): State<AppState>,
    Path(code): Code,
    headers: HeaderMap,
    jar: CookieJar,
) -> Response {
    let (user, jar) = match changing_caller(&state, &headers, jar).await {
        Ok(caller) => caller,
        Err(response) => return *response,
    };
    let result = async {
        let code = invite_code(&code)?;
        let id = state.communities.redeem_invite(&code, &user).await?;
        Ok::<_, Failure>(JoinedDto { community_id: id.to_string() })
    };
    reply(jar, result.await)
}

/// `DELETE /api/invites/{code}`: its creator, or whoever manages channels.
async fn remove(
    State(state): State<AppState>,
    Path(code): Code,
    headers: HeaderMap,
    jar: CookieJar,
) -> Response {
    let (user, jar) = match changing_caller(&state, &headers, jar).await {
        Ok(caller) => caller,
        Err(response) => return *response,
    };
    let result = async {
        let code = invite_code(&code)?;
        Ok::<_, Failure>(state.communities.delete_invite(&code, &user).await?)
    };
    done(jar, result.await)
}
