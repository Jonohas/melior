use async_trait::async_trait;
use melior_application::{RepositoryError, RoomRepository};
use melior_domain::{ChannelId, RoomAccess, User, UserId, Username};
use sqlx::Row;
use sqlx::postgres::{PgPool, PgPoolOptions, PgRow};
use std::time::Duration;
use tokio::sync::OnceCell;

const COLUMNS: &str = "channel_id, owner_id, owner_username, restricted, members";

/// [`RoomRepository`] on Postgres.
///
/// Nothing is connected at construction: the pool connects on first use and the
/// schema migrations run once before the first query, so the server starts while
/// the database is still coming up and reports a useful error per request.
pub struct PostgresRoomRepository {
    pool: PgPool,
    migrated: OnceCell<()>,
}

impl PostgresRoomRepository {
    /// `url` looks like `postgres://user:password@host:port/database`.
    pub fn new(url: &str) -> Result<Self, RepositoryError> {
        let pool = PgPoolOptions::new()
            .max_connections(8)
            .acquire_timeout(Duration::from_secs(5))
            .connect_lazy(url)
            .map_err(repository_error)?;
        Ok(Self { pool, migrated: OnceCell::new() })
    }

    /// The pool, with the schema brought up to date.
    async fn pool(&self) -> Result<&PgPool, RepositoryError> {
        self.migrated
            .get_or_try_init(|| async { sqlx::migrate!("./migrations").run(&self.pool).await })
            .await
            .map_err(repository_error)?;
        Ok(&self.pool)
    }

    fn members(room: &RoomAccess) -> Vec<String> {
        room.members().map(|name| name.as_str().to_owned()).collect()
    }
}

#[async_trait]
impl RoomRepository for PostgresRoomRepository {
    async fn find(&self, channel_id: &ChannelId) -> Result<Option<RoomAccess>, RepositoryError> {
        sqlx::query(&format!("SELECT {COLUMNS} FROM rooms WHERE channel_id = $1"))
            .bind(channel_id.as_str())
            .fetch_optional(self.pool().await?)
            .await
            .map_err(repository_error)?
            .as_ref()
            .map(room_from_row)
            .transpose()
    }

    async fn insert_if_absent(&self, room: &RoomAccess) -> Result<RoomAccess, RepositoryError> {
        let pool = self.pool().await?;
        sqlx::query(
            "INSERT INTO rooms (channel_id, owner_id, owner_username, restricted, members)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (channel_id) DO NOTHING",
        )
        .bind(room.channel_id().as_str())
        .bind(room.owner_id().as_str())
        .bind(room.owner_username())
        .bind(room.is_restricted())
        .bind(Self::members(room))
        .execute(pool)
        .await
        .map_err(repository_error)?;

        // Deleted again in between: then what the caller offered is what holds.
        Ok(self.find(room.channel_id()).await?.unwrap_or_else(|| room.clone()))
    }

    async fn save(&self, room: &RoomAccess) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO rooms (channel_id, owner_id, owner_username, restricted, members)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (channel_id) DO UPDATE SET
                 owner_id = EXCLUDED.owner_id,
                 owner_username = EXCLUDED.owner_username,
                 restricted = EXCLUDED.restricted,
                 members = EXCLUDED.members,
                 updated_at = now()",
        )
        .bind(room.channel_id().as_str())
        .bind(room.owner_id().as_str())
        .bind(room.owner_username())
        .bind(room.is_restricted())
        .bind(Self::members(room))
        .execute(self.pool().await?)
        .await
        .map_err(repository_error)?;
        Ok(())
    }

    async fn delete(&self, channel_id: &ChannelId) -> Result<(), RepositoryError> {
        sqlx::query("DELETE FROM rooms WHERE channel_id = $1")
            .bind(channel_id.as_str())
            .execute(self.pool().await?)
            .await
            .map_err(repository_error)?;
        Ok(())
    }

    async fn list_for(&self, user: &User) -> Result<Vec<RoomAccess>, RepositoryError> {
        // A name that cannot be an invitation matches no invitation.
        let invited_as = Username::parse(&user.username).map(|n| n.as_str().to_owned()).ok();
        sqlx::query(&format!(
            "SELECT {COLUMNS} FROM rooms
             WHERE owner_id = $1 OR $2 = ANY (members)
             ORDER BY channel_id"
        ))
        .bind(user.id.as_str())
        .bind(invited_as)
        .fetch_all(self.pool().await?)
        .await
        .map_err(repository_error)?
        .iter()
        .map(room_from_row)
        .collect()
    }

    async fn list_all(&self) -> Result<Vec<RoomAccess>, RepositoryError> {
        sqlx::query(&format!("SELECT {COLUMNS} FROM rooms ORDER BY channel_id"))
            .fetch_all(self.pool().await?)
            .await
            .map_err(repository_error)?
            .iter()
            .map(room_from_row)
            .collect()
    }
}

/// Rebuilds a room from a row. Stored values go through the same validation as
/// user input: a row that does not fit the domain is an error, not a room.
fn room_from_row(row: &PgRow) -> Result<RoomAccess, RepositoryError> {
    let channel_id: String = row.try_get("channel_id").map_err(repository_error)?;
    let owner_id: String = row.try_get("owner_id").map_err(repository_error)?;
    let owner_username: String = row.try_get("owner_username").map_err(repository_error)?;
    let restricted: bool = row.try_get("restricted").map_err(repository_error)?;
    let members: Vec<String> = row.try_get("members").map_err(repository_error)?;

    let channel_id = ChannelId::parse(&channel_id).map_err(repository_error)?;
    let members = members
        .iter()
        .map(|name| Username::parse(name))
        .collect::<Result<Vec<_>, _>>()
        .map_err(repository_error)?;
    Ok(RoomAccess::restore(channel_id, UserId::new(owner_id), owner_username, restricted, members))
}

fn repository_error(error: impl std::fmt::Display) -> RepositoryError {
    RepositoryError(error.to_string())
}
