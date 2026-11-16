use super::RedisConnection;
use async_trait::async_trait;
use melior_application::{LeaseError, RoomLeases};
use melior_domain::ChannelId;
use std::sync::Arc;
use std::time::Duration;

/// Takes the lease if it is free or already ours, and (re)starts its expiry.
const HOLD: &str = r"
local owner = redis.call('GET', KEYS[1])
if owner == false or owner == ARGV[1] then
    redis.call('SET', KEYS[1], ARGV[1], 'PX', ARGV[2])
    return 1
end
return 0
";

/// [`RoomLeases`] on Redis: one key per room naming the instance that hosts it.
pub struct RedisRoomLeases {
    connection: Arc<RedisConnection>,
    instance_id: String,
    ttl: Duration,
}

impl RedisRoomLeases {
    /// `instance_id` must differ between the instances that share this Redis.
    /// `ttl` is how long a room stays tied to an instance that stopped renewing.
    pub fn new(connection: Arc<RedisConnection>, instance_id: String, ttl: Duration) -> Self {
        Self { connection, instance_id, ttl }
    }

    fn key(channel_id: &ChannelId) -> String {
        format!("room-host:{channel_id}")
    }
}

#[async_trait]
impl RoomLeases for RedisRoomLeases {
    async fn hold(&self, channel_id: &ChannelId) -> Result<bool, LeaseError> {
        let mut connection = self.connection.get().await.map_err(lease_error)?;
        let ttl_ms = u64::try_from(self.ttl.as_millis()).unwrap_or(u64::MAX).max(1);
        let held: i64 = ::redis::Script::new(HOLD)
            .key(Self::key(channel_id))
            .arg(&self.instance_id)
            .arg(ttl_ms)
            .invoke_async(&mut connection)
            .await
            .map_err(lease_error)?;
        Ok(held == 1)
    }
}

fn lease_error(error: impl std::fmt::Display) -> LeaseError {
    LeaseError(error.to_string())
}
