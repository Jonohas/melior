//! Reading communities back. Stored values go through the same validation as user
//! input: a row that does not fit the domain is an error, not a value.

use super::store_error;
use melior_application::CommunityStoreError;
use melior_domain::{
    Category, CategoryId, Channel, ChannelId, ChannelKind, ChannelMedia, Community, CommunityId,
    CommunityState, Invite, InviteCode, MediaMode, Member, Name, Override, OverrideTarget,
    Permissions, Role, RoleId, Topic, UserId,
};
use sqlx::postgres::{PgPool, PgRow};
use sqlx::{Decode, Postgres, Row, Type};
use std::collections::HashMap;

type Result<T> = std::result::Result<T, CommunityStoreError>;

pub(super) const COMMUNITY_COLUMNS: &str = "id, name, owner_id, owner_username";
pub(super) const INVITE_COLUMNS: &str =
    "code, community_id, created_by, creator_id, expires_at_ms, max_uses, uses";
/// A member with their roles, in one row per member.
pub(super) const MEMBER_QUERY: &str = "SELECT m.user_id, m.username,
        COALESCE(array_agg(r.role_id) FILTER (WHERE r.role_id IS NOT NULL), '{}') AS role_ids
    FROM members m
    LEFT JOIN member_roles r ON r.community_id = m.community_id AND r.user_id = m.user_id";

fn get<'r, T>(row: &'r PgRow, column: &str) -> Result<T>
where
    T: Decode<'r, Postgres> + Type<Postgres>,
{
    row.try_get(column).map_err(store_error)
}

fn invalid(what: &str, value: &str) -> CommunityStoreError {
    CommunityStoreError(format!("stored {what} {value:?} is invalid"))
}

fn name(row: &PgRow) -> Result<Name> {
    Name::parse(&get::<String>(row, "name")?).map_err(store_error)
}

fn unsigned(value: i32, what: &str) -> Result<u32> {
    u32::try_from(value).map_err(|_| invalid(what, &value.to_string()))
}

fn permissions(value: i64) -> Result<Permissions> {
    let bits = u64::try_from(value).map_err(|_| invalid("permissions", &value.to_string()))?;
    Ok(Permissions::from_bits(bits))
}

pub(super) fn community_id(raw: &str) -> Result<CommunityId> {
    CommunityId::parse(raw).ok_or_else(|| invalid("community id", raw))
}

pub(super) fn community_from_row(row: &PgRow) -> Result<Community> {
    Ok(Community {
        id: community_id(&get::<String>(row, "id")?)?,
        name: name(row)?,
        owner_id: UserId::new(get::<String>(row, "owner_id")?),
        owner_username: get(row, "owner_username")?,
    })
}

fn role_from_row(row: &PgRow) -> Result<Role> {
    let id: String = get(row, "id")?;
    let color: Option<i32> = get(row, "color")?;
    Ok(Role {
        id: RoleId::parse(&id).ok_or_else(|| invalid("role id", &id))?,
        name: name(row)?,
        color: color.map(|c| unsigned(c, "colour")).transpose()?,
        position: unsigned(get(row, "position")?, "position")?,
        permissions: permissions(get(row, "permissions")?)?,
    })
}

/// `(owner id, override)` from a row of `category_overrides` or `channel_overrides`.
fn override_from_row(row: &PgRow) -> Result<(String, Override)> {
    let kind: String = get(row, "target_kind")?;
    let target_id: String = get(row, "target_id")?;
    let target = match kind.as_str() {
        "role" => OverrideTarget::Role(
            RoleId::parse(&target_id).ok_or_else(|| invalid("role id", &target_id))?,
        ),
        "member" => OverrideTarget::Member(UserId::new(target_id)),
        other => return Err(invalid("override target", other)),
    };
    let allow = permissions(get(row, "allow")?)?;
    let deny = permissions(get(row, "deny")?)?;
    Ok((get(row, "owner")?, Override { target, allow, deny }))
}

fn category_from_row(
    row: &PgRow,
    overrides: &mut HashMap<String, Vec<Override>>,
) -> Result<Category> {
    let id: String = get(row, "id")?;
    Ok(Category {
        overrides: overrides.remove(&id).unwrap_or_default(),
        id: CategoryId::parse(&id).ok_or_else(|| invalid("category id", &id))?,
        name: name(row)?,
        position: unsigned(get(row, "position")?, "position")?,
    })
}

fn channel_from_row(
    row: &PgRow,
    overrides: &mut HashMap<String, Vec<Override>>,
) -> Result<Channel> {
    let id: String = get(row, "id")?;
    let kind: String = get(row, "kind")?;
    let mode: String = get(row, "media_mode")?;
    let category: Option<String> = get(row, "category_id")?;
    let user_limit: Option<i16> = get(row, "user_limit")?;
    Ok(Channel {
        overrides: overrides.remove(&id).unwrap_or_default(),
        id: ChannelId::parse(&id).map_err(store_error)?,
        name: name(row)?,
        topic: Topic::parse(&get::<String>(row, "topic")?).map_err(store_error)?,
        kind: ChannelKind::parse(&kind).ok_or_else(|| invalid("channel kind", &kind))?,
        category_id: category
            .map(|raw| CategoryId::parse(&raw).ok_or_else(|| invalid("category id", &raw)))
            .transpose()?,
        position: unsigned(get(row, "position")?, "position")?,
        synced: get(row, "synced")?,
        media: ChannelMedia {
            mode: MediaMode::parse(&mode).ok_or_else(|| invalid("media mode", &mode))?,
            user_limit: user_limit
                .map(|l| u8::try_from(l).map_err(|_| invalid("user limit", &l.to_string())))
                .transpose()?,
        },
    })
}

pub(super) fn member_from_row(row: &PgRow) -> Result<Member> {
    let role_ids: Vec<String> = get(row, "role_ids")?;
    let role_ids = role_ids
        .iter()
        .map(|raw| RoleId::parse(raw).ok_or_else(|| invalid("role id", raw)))
        .collect::<Result<_>>()?;
    Ok(Member {
        user_id: UserId::new(get::<String>(row, "user_id")?),
        username: get(row, "username")?,
        role_ids,
    })
}

pub(super) fn invite_from_row(row: &PgRow) -> Result<Invite> {
    let code: String = get(row, "code")?;
    let max_uses: Option<i32> = get(row, "max_uses")?;
    Ok(Invite {
        code: InviteCode::parse(&code).ok_or_else(|| invalid("invite code", &code))?,
        community_id: community_id(&get::<String>(row, "community_id")?)?,
        created_by: get(row, "created_by")?,
        creator_id: UserId::new(get::<String>(row, "creator_id")?),
        expires_at_ms: get(row, "expires_at_ms")?,
        max_uses: max_uses.map(|m| unsigned(m, "max uses")).transpose()?,
        uses: unsigned(get(row, "uses")?, "uses")?,
    })
}

async fn overrides_of(
    pool: &PgPool,
    id: &CommunityId,
    table: &str,
    owner: &str,
    parent: &str,
) -> Result<HashMap<String, Vec<Override>>> {
    let rows = sqlx::query(&format!(
        "SELECT o.{owner} AS owner, o.target_kind, o.target_id, o.allow, o.deny
         FROM {table} o JOIN {parent} p ON p.id = o.{owner}
         WHERE p.community_id = $1
         ORDER BY o.target_kind, o.target_id"
    ))
    .bind(id.as_str())
    .fetch_all(pool)
    .await
    .map_err(store_error)?;
    let mut grouped: HashMap<String, Vec<Override>> = HashMap::new();
    for row in &rows {
        let (owner, entry) = override_from_row(row)?;
        grouped.entry(owner).or_default().push(entry);
    }
    Ok(grouped)
}

async fn rows(pool: &PgPool, sql: &str, id: &CommunityId) -> Result<Vec<PgRow>> {
    sqlx::query(sql).bind(id.as_str()).fetch_all(pool).await.map_err(store_error)
}

/// A community with its whole structure, in a fixed number of queries.
pub(super) async fn load(pool: &PgPool, id: &CommunityId) -> Result<Option<CommunityState>> {
    let sql = format!("SELECT {COMMUNITY_COLUMNS} FROM communities WHERE id = $1");
    let row =
        sqlx::query(&sql).bind(id.as_str()).fetch_optional(pool).await.map_err(store_error)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let community = community_from_row(&row)?;
    let roles = rows(
        pool,
        "SELECT id, name, color, position, permissions FROM roles WHERE community_id = $1",
        id,
    )
    .await?
    .iter()
    .map(role_from_row)
    .collect::<Result<_>>()?;
    let mut category_overrides =
        overrides_of(pool, id, "category_overrides", "category_id", "categories").await?;
    let categories =
        rows(pool, "SELECT id, name, position FROM categories WHERE community_id = $1", id)
            .await?
            .iter()
            .map(|row| category_from_row(row, &mut category_overrides))
            .collect::<Result<_>>()?;
    let mut channel_overrides =
        overrides_of(pool, id, "channel_overrides", "channel_id", "channels").await?;
    let channels = rows(
        pool,
        "SELECT id, name, topic, kind, category_id, position, synced, media_mode, user_limit
         FROM channels WHERE community_id = $1",
        id,
    )
    .await?
    .iter()
    .map(|row| channel_from_row(row, &mut channel_overrides))
    .collect::<Result<_>>()?;
    Ok(Some(CommunityState { community, roles, categories, channels }))
}
