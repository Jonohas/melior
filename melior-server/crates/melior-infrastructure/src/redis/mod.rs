//! Redis adapters: login sessions and room leases.

mod leases;
mod sessions;

pub use leases::RedisRoomLeases;
pub use sessions::RedisSessionStore;

use ::redis::aio::ConnectionManager;
use tokio::sync::OnceCell;

/// A Redis connection shared by the adapters. It is opened on first use, so the
/// server starts (and reports a useful error per request) while Redis is still
/// coming up, and it reconnects by itself afterwards.
pub struct RedisConnection {
    client: ::redis::Client,
    manager: OnceCell<ConnectionManager>,
}

impl RedisConnection {
    /// `url` looks like `redis://[:password@]host:port[/db]`.
    pub fn new(url: &str) -> Result<Self, ::redis::RedisError> {
        Ok(Self { client: ::redis::Client::open(url)?, manager: OnceCell::new() })
    }

    async fn get(&self) -> Result<ConnectionManager, ::redis::RedisError> {
        self.manager.get_or_try_init(|| ConnectionManager::new(self.client.clone())).await.cloned()
    }
}
