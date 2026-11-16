//! What the room registry needs from the outside world.

use async_trait::async_trait;
use melior_domain::{ChannelId, RoomAccess, User};

#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
#[error("room repository: {0}")]
pub struct RepositoryError(pub String);

/// Durable storage for [`RoomAccess`] entries, keyed by channel id.
#[async_trait]
pub trait RoomRepository: Send + Sync {
    async fn find(&self, channel_id: &ChannelId) -> Result<Option<RoomAccess>, RepositoryError>;

    /// Stores `room` unless its channel is already registered. Returns what is
    /// stored afterwards, so two people creating a room at once agree on its owner.
    async fn insert_if_absent(&self, room: &RoomAccess) -> Result<RoomAccess, RepositoryError>;

    /// Stores `room`, replacing an existing entry for the same channel.
    async fn save(&self, room: &RoomAccess) -> Result<(), RepositoryError>;

    async fn delete(&self, channel_id: &ChannelId) -> Result<(), RepositoryError>;

    /// The rooms `user` owns or is invited to, by channel id.
    async fn list_for(&self, user: &User) -> Result<Vec<RoomAccess>, RepositoryError>;

    /// Every room, by channel id.
    async fn list_all(&self) -> Result<Vec<RoomAccess>, RepositoryError>;
}
