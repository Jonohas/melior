//! Writing communities: every write of several rows is one statement over arrays,
//! never a statement per row.

use super::store_error;
use melior_application::CommunityStoreError;
use melior_domain::{Category, Channel, CommunityId, Override, OverrideTarget, Role};
use sqlx::PgConnection;

type Result<T> = std::result::Result<T, CommunityStoreError>;

fn bits(permissions: melior_domain::Permissions) -> i64 {
    i64::try_from(permissions.bits()).unwrap_or(i64::MAX)
}

fn signed(value: u32) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

/// Inserts or replaces roles.
pub(super) async fn upsert_roles(
    connection: &mut PgConnection,
    id: &CommunityId,
    roles: &[Role],
) -> Result<()> {
    let ids: Vec<&str> = roles.iter().map(|r| r.id.as_str()).collect();
    let names: Vec<&str> = roles.iter().map(|r| r.name.as_str()).collect();
    let colors: Vec<Option<i32>> = roles.iter().map(|r| r.color.map(signed)).collect();
    let positions: Vec<i32> = roles.iter().map(|r| signed(r.position)).collect();
    let permissions: Vec<i64> = roles.iter().map(|r| bits(r.permissions)).collect();
    sqlx::query(
        "INSERT INTO roles (id, community_id, name, color, position, permissions)
         SELECT r.id, $1, r.name, r.color, r.position, r.permissions
         FROM unnest($2::text[], $3::text[], $4::int[], $5::int[], $6::bigint[])
             AS r (id, name, color, position, permissions)
         ON CONFLICT (id) DO UPDATE SET
             name = EXCLUDED.name, color = EXCLUDED.color,
             position = EXCLUDED.position, permissions = EXCLUDED.permissions",
    )
    .bind(id.as_str())
    .bind(ids)
    .bind(names)
    .bind(colors)
    .bind(positions)
    .bind(permissions)
    .execute(connection)
    .await
    .map_err(store_error)?;
    Ok(())
}

/// Inserts or replaces categories, with their overrides.
pub(super) async fn upsert_categories(
    connection: &mut PgConnection,
    id: &CommunityId,
    categories: &[Category],
) -> Result<()> {
    let ids: Vec<&str> = categories.iter().map(|c| c.id.as_str()).collect();
    let names: Vec<&str> = categories.iter().map(|c| c.name.as_str()).collect();
    let positions: Vec<i32> = categories.iter().map(|c| signed(c.position)).collect();
    sqlx::query(
        "INSERT INTO categories (id, community_id, name, position)
         SELECT c.id, $1, c.name, c.position
         FROM unnest($2::text[], $3::text[], $4::int[]) AS c (id, name, position)
         ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name, position = EXCLUDED.position",
    )
    .bind(id.as_str())
    .bind(ids)
    .bind(names)
    .bind(positions)
    .execute(&mut *connection)
    .await
    .map_err(store_error)?;
    let owned: Vec<(&str, &[Override])> =
        categories.iter().map(|c| (c.id.as_str(), c.overrides.as_slice())).collect();
    replace_overrides(connection, Overrides::CATEGORY, &owned).await
}

/// Inserts or replaces channels, with their overrides.
pub(super) async fn upsert_channels(
    connection: &mut PgConnection,
    id: &CommunityId,
    channels: &[Channel],
) -> Result<()> {
    let ids: Vec<&str> = channels.iter().map(|c| c.id.as_str()).collect();
    let names: Vec<&str> = channels.iter().map(|c| c.name.as_str()).collect();
    let topics: Vec<&str> = channels.iter().map(|c| c.topic.as_str()).collect();
    let kinds: Vec<&str> = channels.iter().map(|c| c.kind.as_str()).collect();
    let categories: Vec<Option<&str>> = channels
        .iter()
        .map(|c| c.category_id.as_ref().map(melior_domain::CategoryId::as_str))
        .collect();
    let positions: Vec<i32> = channels.iter().map(|c| signed(c.position)).collect();
    let synced: Vec<bool> = channels.iter().map(|c| c.synced).collect();
    let modes: Vec<&str> = channels.iter().map(|c| c.media.mode.as_str()).collect();
    let limits: Vec<Option<i16>> =
        channels.iter().map(|c| c.media.user_limit.map(i16::from)).collect();
    sqlx::query(
        "INSERT INTO channels
             (id, community_id, name, topic, kind, category_id, position, synced, media_mode, user_limit)
         SELECT c.id, $1, c.name, c.topic, c.kind, c.category_id, c.position, c.synced, c.mode, c.lim
         FROM unnest($2::text[], $3::text[], $4::text[], $5::text[], $6::text[], $7::int[],
                     $8::bool[], $9::text[], $10::smallint[])
             AS c (id, name, topic, kind, category_id, position, synced, mode, lim)
         ON CONFLICT (id) DO UPDATE SET
             name = EXCLUDED.name, topic = EXCLUDED.topic, category_id = EXCLUDED.category_id,
             position = EXCLUDED.position, synced = EXCLUDED.synced,
             media_mode = EXCLUDED.media_mode, user_limit = EXCLUDED.user_limit,
             updated_at = now()",
    )
    .bind(id.as_str())
    .bind(ids)
    .bind(names)
    .bind(topics)
    .bind(kinds)
    .bind(categories)
    .bind(positions)
    .bind(synced)
    .bind(modes)
    .bind(limits)
    .execute(&mut *connection)
    .await
    .map_err(store_error)?;
    let owned: Vec<(&str, &[Override])> =
        channels.iter().map(|c| (c.id.as_str(), c.overrides.as_slice())).collect();
    replace_overrides(connection, Overrides::CHANNEL, &owned).await
}

/// Where the overrides of one kind of owner live.
pub(super) struct Overrides {
    table: &'static str,
    owner: &'static str,
}

impl Overrides {
    pub(super) const CATEGORY: Self = Self { table: "category_overrides", owner: "category_id" };
    pub(super) const CHANNEL: Self = Self { table: "channel_overrides", owner: "channel_id" };
}

/// Replaces the overrides of every owner given with the ones given.
async fn replace_overrides(
    connection: &mut PgConnection,
    place: Overrides,
    owned: &[(&str, &[Override])],
) -> Result<()> {
    let Overrides { table, owner } = place;
    let owners: Vec<&str> = owned.iter().map(|(id, _)| *id).collect();
    sqlx::query(&format!("DELETE FROM {table} WHERE {owner} = ANY ($1)"))
        .bind(owners)
        .execute(&mut *connection)
        .await
        .map_err(store_error)?;

    let rows: Vec<(&str, &Override)> =
        owned.iter().flat_map(|(id, list)| list.iter().map(move |entry| (*id, entry))).collect();
    if rows.is_empty() {
        return Ok(());
    }
    let owner_ids: Vec<&str> = rows.iter().map(|(id, _)| *id).collect();
    let kinds: Vec<&str> = rows.iter().map(|(_, entry)| target(entry).0).collect();
    let targets: Vec<&str> = rows.iter().map(|(_, entry)| target(entry).1).collect();
    let allows: Vec<i64> = rows.iter().map(|(_, entry)| bits(entry.allow)).collect();
    let denies: Vec<i64> = rows.iter().map(|(_, entry)| bits(entry.deny)).collect();
    sqlx::query(&format!(
        "INSERT INTO {table} ({owner}, target_kind, target_id, allow, deny)
         SELECT * FROM unnest($1::text[], $2::text[], $3::text[], $4::bigint[], $5::bigint[])"
    ))
    .bind(owner_ids)
    .bind(kinds)
    .bind(targets)
    .bind(allows)
    .bind(denies)
    .execute(connection)
    .await
    .map_err(store_error)?;
    Ok(())
}

/// `(target_kind, target_id)` as stored.
fn target(entry: &Override) -> (&'static str, &str) {
    match &entry.target {
        OverrideTarget::Role(id) => ("role", id.as_str()),
        OverrideTarget::Member(id) => ("member", id.as_str()),
    }
}
