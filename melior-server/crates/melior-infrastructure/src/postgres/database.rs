use sqlx::postgres::{PgPool, PgPoolOptions};
use std::time::Duration;
use tokio::sync::OnceCell;

/// The Postgres connection every repository shares.
///
/// Nothing is connected at construction: the pool connects on first use and the
/// schema migrations run once before the first query, so the server starts while
/// the database is still coming up and reports a useful error per request.
pub struct Database {
    pool: PgPool,
    migrated: OnceCell<()>,
}

impl Database {
    /// `url` looks like `postgres://user:password@host:port/database`.
    pub fn new(url: &str) -> Result<Self, sqlx::Error> {
        let pool = PgPoolOptions::new()
            .max_connections(8)
            .acquire_timeout(Duration::from_secs(5))
            .connect_lazy(url)?;
        Ok(Self { pool, migrated: OnceCell::new() })
    }

    /// The pool, with the schema brought up to date.
    pub(super) async fn pool(&self) -> Result<&PgPool, sqlx::Error> {
        self.migrated
            .get_or_try_init(|| async { sqlx::migrate!("./migrations").run(&self.pool).await })
            .await?;
        Ok(&self.pool)
    }
}
