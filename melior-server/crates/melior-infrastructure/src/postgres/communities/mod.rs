//! [`CommunityRepository`] on Postgres.

mod invites;
mod read;
mod write;

use super::Database;
use async_trait::async_trait;
use melior_application::{CommunityRepository, CommunityStoreError, Redemption};
use melior_domain::{
    Category, CategoryId, Channel, ChannelId, Community, CommunityId, CommunityState, Invite,
    InviteCode, Member, Role, RoleId, UserId,
};
use read::{COMMUNITY_COLUMNS, MEMBER_QUERY, community_from_row, member_from_row};
use sqlx::postgres::PgPool;
use std::sync::Arc;

type Result<T> = std::result::Result<T, CommunityStoreError>;

fn store_error(error: impl std::fmt::Display) -> CommunityStoreError {
    CommunityStoreError(error.to_string())
}

/// [`CommunityRepository`] on Postgres.
pub struct PostgresCommunityRepository {
    database: Arc<Database>,
}

impl PostgresCommunityRepository {
    #[must_use]
    pub fn new(database: Arc<Database>) -> Self {
        Self { database }
    }

    /// The pool, with the schema brought up to date.
    async fn pool(&self) -> Result<&PgPool> {
        self.database.pool().await.map_err(store_error)
    }

    async fn execute(&self, sql: &str, binds: &[&str]) -> Result<()> {
        let mut query = sqlx::query(sql);
        for value in binds {
            query = query.bind(*value);
        }
        query.execute(self.pool().await?).await.map_err(store_error)?;
        Ok(())
    }
}

#[async_trait]
impl CommunityRepository for PostgresCommunityRepository {
    async fn create(&self, state: &CommunityState, owner: &Member) -> Result<()> {
        let mut tx = self.pool().await?.begin().await.map_err(store_error)?;
        let community = &state.community;
        sqlx::query(
            "INSERT INTO communities (id, name, owner_id, owner_username) VALUES ($1, $2, $3, $4)",
        )
        .bind(community.id.as_str())
        .bind(community.name.as_str())
        .bind(community.owner_id.as_str())
        .bind(&community.owner_username)
        .execute(&mut *tx)
        .await
        .map_err(store_error)?;
        sqlx::query("INSERT INTO members (community_id, user_id, username) VALUES ($1, $2, $3)")
            .bind(community.id.as_str())
            .bind(owner.user_id.as_str())
            .bind(&owner.username)
            .execute(&mut *tx)
            .await
            .map_err(store_error)?;
        write::upsert_roles(&mut tx, state.id(), &state.roles).await?;
        write::upsert_categories(&mut tx, state.id(), &state.categories).await?;
        write::upsert_channels(&mut tx, state.id(), &state.channels).await?;
        tx.commit().await.map_err(store_error)
    }

    async fn load(&self, id: &CommunityId) -> Result<Option<CommunityState>> {
        read::load(self.pool().await?, id).await
    }

    async fn load_by_channel(&self, channel_id: &ChannelId) -> Result<Option<CommunityState>> {
        let pool = self.pool().await?;
        let community: Option<String> =
            sqlx::query_scalar("SELECT community_id FROM channels WHERE id = $1")
                .bind(channel_id.as_str())
                .fetch_optional(pool)
                .await
                .map_err(store_error)?;
        let Some(community) = community else {
            return Ok(None);
        };
        read::load(pool, &read::community_id(&community)?).await
    }

    async fn list_for(&self, user: &UserId) -> Result<Vec<Community>> {
        sqlx::query(&format!(
            "SELECT {COMMUNITY_COLUMNS} FROM communities
             WHERE id IN (SELECT community_id FROM members WHERE user_id = $1)
             ORDER BY name, id"
        ))
        .bind(user.as_str())
        .fetch_all(self.pool().await?)
        .await
        .map_err(store_error)?
        .iter()
        .map(community_from_row)
        .collect()
    }

    async fn rename(&self, community: &Community) -> Result<()> {
        let sql = "UPDATE communities SET name = $2 WHERE id = $1";
        self.execute(sql, &[community.id.as_str(), community.name.as_str()]).await
    }

    async fn delete(&self, id: &CommunityId) -> Result<()> {
        self.execute("DELETE FROM communities WHERE id = $1", &[id.as_str()]).await
    }

    async fn member(&self, id: &CommunityId, user: &UserId) -> Result<Option<Member>> {
        sqlx::query(&format!(
            "{MEMBER_QUERY} WHERE m.community_id = $1 AND m.user_id = $2
             GROUP BY m.user_id, m.username"
        ))
        .bind(id.as_str())
        .bind(user.as_str())
        .fetch_optional(self.pool().await?)
        .await
        .map_err(store_error)?
        .as_ref()
        .map(member_from_row)
        .transpose()
    }

    async fn members(&self, id: &CommunityId) -> Result<Vec<Member>> {
        sqlx::query(&format!(
            "{MEMBER_QUERY} WHERE m.community_id = $1
             GROUP BY m.user_id, m.username ORDER BY m.username, m.user_id"
        ))
        .bind(id.as_str())
        .fetch_all(self.pool().await?)
        .await
        .map_err(store_error)?
        .iter()
        .map(member_from_row)
        .collect()
    }

    async fn member_count(&self, id: &CommunityId) -> Result<u64> {
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM members WHERE community_id = $1")
            .bind(id.as_str())
            .fetch_one(self.pool().await?)
            .await
            .map_err(store_error)?;
        Ok(u64::try_from(count).unwrap_or(0))
    }

    async fn remove_member(&self, id: &CommunityId, user: &UserId) -> Result<()> {
        let sql = "DELETE FROM members WHERE community_id = $1 AND user_id = $2";
        self.execute(sql, &[id.as_str(), user.as_str()]).await
    }

    async fn set_member_roles(&self, id: &CommunityId, member: &Member) -> Result<()> {
        let mut tx = self.pool().await?.begin().await.map_err(store_error)?;
        sqlx::query("DELETE FROM member_roles WHERE community_id = $1 AND user_id = $2")
            .bind(id.as_str())
            .bind(member.user_id.as_str())
            .execute(&mut *tx)
            .await
            .map_err(store_error)?;
        let roles: Vec<&str> = member.role_ids.iter().map(RoleId::as_str).collect();
        sqlx::query(
            "INSERT INTO member_roles (community_id, user_id, role_id)
             SELECT $1, $2, role_id FROM unnest($3::text[]) AS role_id",
        )
        .bind(id.as_str())
        .bind(member.user_id.as_str())
        .bind(roles)
        .execute(&mut *tx)
        .await
        .map_err(store_error)?;
        tx.commit().await.map_err(store_error)
    }

    async fn save_role(&self, id: &CommunityId, role: &Role) -> Result<()> {
        let mut connection = self.pool().await?.acquire().await.map_err(store_error)?;
        write::upsert_roles(&mut connection, id, std::slice::from_ref(role)).await
    }

    async fn delete_role(&self, id: &CommunityId, role: &RoleId) -> Result<()> {
        let mut tx = self.pool().await?.begin().await.map_err(store_error)?;
        for sql in [
            "DELETE FROM channel_overrides WHERE target_kind = 'role' AND target_id = $2
               AND channel_id IN (SELECT id FROM channels WHERE community_id = $1)",
            "DELETE FROM category_overrides WHERE target_kind = 'role' AND target_id = $2
               AND category_id IN (SELECT id FROM categories WHERE community_id = $1)",
            "DELETE FROM roles WHERE community_id = $1 AND id = $2",
        ] {
            sqlx::query(sql)
                .bind(id.as_str())
                .bind(role.as_str())
                .execute(&mut *tx)
                .await
                .map_err(store_error)?;
        }
        tx.commit().await.map_err(store_error)
    }

    async fn save_category(&self, id: &CommunityId, category: &Category) -> Result<()> {
        let mut tx = self.pool().await?.begin().await.map_err(store_error)?;
        write::upsert_categories(&mut tx, id, std::slice::from_ref(category)).await?;
        tx.commit().await.map_err(store_error)
    }

    async fn delete_category(&self, id: &CommunityId, category: &CategoryId) -> Result<()> {
        let sql = "DELETE FROM categories WHERE community_id = $1 AND id = $2";
        self.execute(sql, &[id.as_str(), category.as_str()]).await
    }

    async fn save_channel(&self, id: &CommunityId, channel: &Channel) -> Result<()> {
        let mut tx = self.pool().await?.begin().await.map_err(store_error)?;
        write::upsert_channels(&mut tx, id, std::slice::from_ref(channel)).await?;
        tx.commit().await.map_err(store_error)
    }

    async fn delete_channel(&self, channel: &ChannelId) -> Result<()> {
        self.execute("DELETE FROM channels WHERE id = $1", &[channel.as_str()]).await
    }

    async fn save_invite(&self, invite: &Invite) -> Result<()> {
        invites::save(self.pool().await?, invite).await
    }

    async fn invite(&self, code: &InviteCode) -> Result<Option<Invite>> {
        invites::find(self.pool().await?, code).await
    }

    async fn invites(&self, id: &CommunityId) -> Result<Vec<Invite>> {
        invites::of_community(self.pool().await?, id).await
    }

    async fn delete_invite(&self, code: &InviteCode) -> Result<()> {
        self.execute("DELETE FROM invites WHERE code = $1", &[code.as_str()]).await
    }

    async fn redeem_invite(
        &self,
        code: &InviteCode,
        member: &Member,
        now_ms: i64,
    ) -> Result<Redemption> {
        invites::redeem(self.pool().await?, code, member, now_ms).await
    }
}
