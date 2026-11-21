//! `/api/communities/...`: communities, their members, roles, categories, new
//! channels and invites.

use super::community_dto::{
    CategoryDto, ChannelViewDto, CommunitySummaryDto, CommunityViewDto, InviteDto, MemberDto,
    RoleDto,
};
use super::community_input::{
    CreateChannelDto, CreateInviteDto, CreateRoleDto, EditCategoryDto, EditRoleDto, MemberRolesDto,
    NameDto,
};
use super::support::{
    Failure, body, caller, category_id, changing_caller, community_id, done, reply, role_id,
};
use crate::AppState;
use axum::Router;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::{delete, get, patch, post, put};
use axum_extra::extract::cookie::CookieJar;
use melior_domain::UserId;

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/communities", get(list).post(create))
        .route("/api/communities/{id}", get(show).patch(rename).delete(remove))
        .route("/api/communities/{id}/members", get(members))
        .route("/api/communities/{id}/members/{user_id}", delete(remove_member))
        .route("/api/communities/{id}/members/{user_id}/roles", put(member_roles))
        .route("/api/communities/{id}/roles", post(create_role))
        .route("/api/communities/{id}/roles/{role_id}", patch(edit_role).delete(delete_role))
        .route("/api/communities/{id}/categories", post(create_category))
        .route(
            "/api/communities/{id}/categories/{category_id}",
            patch(edit_category).delete(delete_category),
        )
        .route("/api/communities/{id}/channels", post(create_channel))
        .route("/api/communities/{id}/invites", get(invites).post(create_invite))
}

type Id = Path<String>;
type Ids = Path<(String, String)>;

/// `GET /api/communities`: the communities the caller is a member of.
async fn list(State(state): State<AppState>, jar: CookieJar) -> Response {
    let (user, jar) = match caller(&state, jar).await {
        Ok(caller) => caller,
        Err(response) => return *response,
    };
    let result = state.communities.list(&user).await.map_err(Failure::from);
    reply(
        jar,
        result.map(|list| list.into_iter().map(CommunitySummaryDto::from).collect::<Vec<_>>()),
    )
}

/// `POST /api/communities` `{ name }`: a new community owned by the caller.
async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
    bytes: Bytes,
) -> Response {
    let (user, jar) = match changing_caller(&state, &headers, jar).await {
        Ok(caller) => caller,
        Err(response) => return *response,
    };
    let result = async {
        let request: NameDto = body(&bytes)?;
        Ok::<_, Failure>(CommunityViewDto::from(
            state.communities.create(&user, &request.name).await?,
        ))
    };
    reply(jar, result.await)
}

/// `GET /api/communities/{id}`: the community as the caller sees it.
async fn show(State(state): State<AppState>, Path(id): Id, jar: CookieJar) -> Response {
    let (user, jar) = match caller(&state, jar).await {
        Ok(caller) => caller,
        Err(response) => return *response,
    };
    let result = async {
        let id = community_id(&id)?;
        Ok::<_, Failure>(CommunityViewDto::from(state.communities.view(&id, &user).await?))
    };
    reply(jar, result.await)
}

/// `PATCH /api/communities/{id}` `{ name }`: the owner or an administrator.
async fn rename(
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
        let id = community_id(&id)?;
        let request: NameDto = body(&bytes)?;
        Ok::<_, Failure>(CommunityViewDto::from(
            state.communities.rename(&id, &user, &request.name).await?,
        ))
    };
    reply(jar, result.await)
}

/// `DELETE /api/communities/{id}`: the owner only.
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
        let id = community_id(&id)?;
        Ok::<_, Failure>(state.communities.delete(&id, &user).await?)
    };
    done(jar, result.await)
}

/// `GET /api/communities/{id}/members`.
async fn members(State(state): State<AppState>, Path(id): Id, jar: CookieJar) -> Response {
    let (user, jar) = match caller(&state, jar).await {
        Ok(caller) => caller,
        Err(response) => return *response,
    };
    let result = async {
        let id = community_id(&id)?;
        let members = state.communities.members(&id, &user).await?;
        Ok::<_, Failure>(members.into_iter().map(MemberDto::from).collect::<Vec<_>>())
    };
    reply(jar, result.await)
}

/// `DELETE /api/communities/{id}/members/{user_id}`: a kick, or leaving when the
/// user id is the caller's.
async fn remove_member(
    State(state): State<AppState>,
    Path((id, target)): Ids,
    headers: HeaderMap,
    jar: CookieJar,
) -> Response {
    let (user, jar) = match changing_caller(&state, &headers, jar).await {
        Ok(caller) => caller,
        Err(response) => return *response,
    };
    let result = async {
        let id = community_id(&id)?;
        Ok::<_, Failure>(state.communities.remove_member(&id, &user, &UserId::new(target)).await?)
    };
    done(jar, result.await)
}

/// `PUT /api/communities/{id}/members/{user_id}/roles` `{ role_ids }`.
async fn member_roles(
    State(state): State<AppState>,
    Path((id, target)): Ids,
    headers: HeaderMap,
    jar: CookieJar,
    bytes: Bytes,
) -> Response {
    let (user, jar) = match changing_caller(&state, &headers, jar).await {
        Ok(caller) => caller,
        Err(response) => return *response,
    };
    let result = async {
        let id = community_id(&id)?;
        let roles = body::<MemberRolesDto>(&bytes)?.into_ids()?;
        let member =
            state.communities.set_member_roles(&id, &user, &UserId::new(target), roles).await?;
        Ok::<_, Failure>(MemberDto::from(member))
    };
    reply(jar, result.await)
}

/// `POST /api/communities/{id}/roles` `{ name, color?, permissions }`.
async fn create_role(
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
        let id = community_id(&id)?;
        let draft = body::<CreateRoleDto>(&bytes)?.into_draft()?;
        Ok::<_, Failure>(RoleDto::from(state.communities.create_role(&id, &user, draft).await?))
    };
    reply(jar, result.await)
}

/// `PATCH /api/communities/{id}/roles/{role_id}` `{ name?, color?, permissions?, position? }`.
async fn edit_role(
    State(state): State<AppState>,
    Path((id, role)): Ids,
    headers: HeaderMap,
    jar: CookieJar,
    bytes: Bytes,
) -> Response {
    let (user, jar) = match changing_caller(&state, &headers, jar).await {
        Ok(caller) => caller,
        Err(response) => return *response,
    };
    let result = async {
        let (id, role) = (community_id(&id)?, role_id(&role)?);
        let changes = body::<EditRoleDto>(&bytes)?.into_changes()?;
        Ok::<_, Failure>(RoleDto::from(
            state.communities.edit_role(&id, &user, &role, changes).await?,
        ))
    };
    reply(jar, result.await)
}

/// `DELETE /api/communities/{id}/roles/{role_id}`: never `@everyone`.
async fn delete_role(
    State(state): State<AppState>,
    Path((id, role)): Ids,
    headers: HeaderMap,
    jar: CookieJar,
) -> Response {
    let (user, jar) = match changing_caller(&state, &headers, jar).await {
        Ok(caller) => caller,
        Err(response) => return *response,
    };
    let result = async {
        let (id, role) = (community_id(&id)?, role_id(&role)?);
        Ok::<_, Failure>(state.communities.delete_role(&id, &user, &role).await?)
    };
    done(jar, result.await)
}

/// `POST /api/communities/{id}/categories` `{ name }`.
async fn create_category(
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
        let id = community_id(&id)?;
        let request: NameDto = body(&bytes)?;
        Ok::<_, Failure>(CategoryDto::from(
            state.communities.create_category(&id, &user, &request.name).await?,
        ))
    };
    reply(jar, result.await)
}

/// `PATCH /api/communities/{id}/categories/{category_id}` `{ name?, position?, overrides? }`.
async fn edit_category(
    State(state): State<AppState>,
    Path((id, category)): Ids,
    headers: HeaderMap,
    jar: CookieJar,
    bytes: Bytes,
) -> Response {
    let (user, jar) = match changing_caller(&state, &headers, jar).await {
        Ok(caller) => caller,
        Err(response) => return *response,
    };
    let result = async {
        let (id, category) = (community_id(&id)?, category_id(&category)?);
        let changes = body::<EditCategoryDto>(&bytes)?.into_changes()?;
        let edited = state.communities.edit_category(&id, &user, &category, changes).await?;
        Ok::<_, Failure>(CategoryDto::from(edited))
    };
    reply(jar, result.await)
}

/// `DELETE /api/communities/{id}/categories/{category_id}`: its channels stay.
async fn delete_category(
    State(state): State<AppState>,
    Path((id, category)): Ids,
    headers: HeaderMap,
    jar: CookieJar,
) -> Response {
    let (user, jar) = match changing_caller(&state, &headers, jar).await {
        Ok(caller) => caller,
        Err(response) => return *response,
    };
    let result = async {
        let (id, category) = (community_id(&id)?, category_id(&category)?);
        Ok::<_, Failure>(state.communities.delete_category(&id, &user, &category).await?)
    };
    done(jar, result.await)
}

/// `POST /api/communities/{id}/channels` `{ name, kind, topic?, category_id?, media_mode?, user_limit? }`.
async fn create_channel(
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
        let id = community_id(&id)?;
        let draft = body::<CreateChannelDto>(&bytes)?.into_draft()?;
        Ok::<_, Failure>(ChannelViewDto::from(
            state.communities.create_channel(&id, &user, draft).await?,
        ))
    };
    reply(jar, result.await)
}

/// `POST /api/communities/{id}/invites` `{ max_age_seconds?, max_uses? }`.
async fn create_invite(
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
        let id = community_id(&id)?;
        let settings = body::<CreateInviteDto>(&bytes)?.into_settings()?;
        Ok::<_, Failure>(InviteDto::from(
            state.communities.create_invite(&id, &user, settings).await?,
        ))
    };
    reply(jar, result.await)
}

/// `GET /api/communities/{id}/invites`: for whoever manages channels.
async fn invites(State(state): State<AppState>, Path(id): Id, jar: CookieJar) -> Response {
    let (user, jar) = match caller(&state, jar).await {
        Ok(caller) => caller,
        Err(response) => return *response,
    };
    let result = async {
        let id = community_id(&id)?;
        let invites = state.communities.invites(&id, &user).await?;
        Ok::<_, Failure>(invites.into_iter().map(InviteDto::from).collect::<Vec<_>>())
    };
    reply(jar, result.await)
}
