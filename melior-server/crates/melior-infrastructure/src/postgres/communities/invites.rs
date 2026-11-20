//! Invites, and redeeming them atomically.

use super::read::{INVITE_COLUMNS, invite_from_row};
use super::{Result, store_error};
use melior_application::Redemption;
use melior_domain::{CommunityId, Invite, InviteCode, Member};
use sqlx::postgres::PgPool;

pub(super) async fn save(pool: &PgPool, invite: &Invite) -> Result<()> {
    let max_uses = invite.max_uses.map(|m| i32::try_from(m).unwrap_or(i32::MAX));
    let uses = i32::try_from(invite.uses).unwrap_or(i32::MAX);
    sqlx::query(&format!(
        "INSERT INTO invites ({INVITE_COLUMNS}) VALUES ($1, $2, $3, $4, $5, $6, $7)"
    ))
    .bind(invite.code.as_str())
    .bind(invite.community_id.as_str())
    .bind(&invite.created_by)
    .bind(invite.creator_id.as_str())
    .bind(invite.expires_at_ms)
    .bind(max_uses)
    .bind(uses)
    .execute(pool)
    .await
    .map_err(store_error)?;
    Ok(())
}

pub(super) async fn find(pool: &PgPool, code: &InviteCode) -> Result<Option<Invite>> {
    sqlx::query(&format!("SELECT {INVITE_COLUMNS} FROM invites WHERE code = $1"))
        .bind(code.as_str())
        .fetch_optional(pool)
        .await
        .map_err(store_error)?
        .as_ref()
        .map(invite_from_row)
        .transpose()
}

pub(super) async fn of_community(pool: &PgPool, id: &CommunityId) -> Result<Vec<Invite>> {
    sqlx::query(&format!(
        "SELECT {INVITE_COLUMNS} FROM invites WHERE community_id = $1 ORDER BY created_at, code"
    ))
    .bind(id.as_str())
    .fetch_all(pool)
    .await
    .map_err(store_error)?
    .iter()
    .map(invite_from_row)
    .collect()
}

/// In one transaction: a member already in the community stays without using the
/// invite; otherwise one conditional `UPDATE` takes a use, so two redeemers can
/// never exceed `max_uses`, and only then is the member added.
pub(super) async fn redeem(
    pool: &PgPool,
    code: &InviteCode,
    member: &Member,
    now_ms: i64,
) -> Result<Redemption> {
    let mut tx = pool.begin().await.map_err(store_error)?;
    let community: Option<String> =
        sqlx::query_scalar("SELECT community_id FROM invites WHERE code = $1")
            .bind(code.as_str())
            .fetch_optional(&mut *tx)
            .await
            .map_err(store_error)?;
    let Some(community) = community else {
        return Ok(Redemption::Unknown);
    };
    let id = super::read::community_id(&community)?;
    let already: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM members WHERE community_id = $1 AND user_id = $2)",
    )
    .bind(id.as_str())
    .bind(member.user_id.as_str())
    .fetch_one(&mut *tx)
    .await
    .map_err(store_error)?;
    if already {
        return Ok(Redemption::AlreadyMember(id));
    }
    let used = sqlx::query(
        "UPDATE invites SET uses = uses + 1
         WHERE code = $1
           AND (max_uses IS NULL OR uses < max_uses)
           AND (expires_at_ms IS NULL OR expires_at_ms > $2)
         RETURNING code",
    )
    .bind(code.as_str())
    .bind(now_ms)
    .fetch_optional(&mut *tx)
    .await
    .map_err(store_error)?;
    if used.is_none() {
        return Ok(Redemption::NotUsable);
    }
    let added = sqlx::query(
        "INSERT INTO members (community_id, user_id, username) VALUES ($1, $2, $3)
         ON CONFLICT DO NOTHING",
    )
    .bind(id.as_str())
    .bind(member.user_id.as_str())
    .bind(&member.username)
    .execute(&mut *tx)
    .await
    .map_err(store_error)?;
    if added.rows_affected() == 0 {
        // Joined by another request in the meantime: give the use back.
        return Ok(Redemption::AlreadyMember(id));
    }
    tx.commit().await.map_err(store_error)?;
    Ok(Redemption::Joined(id))
}
