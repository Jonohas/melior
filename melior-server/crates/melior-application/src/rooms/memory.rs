use super::ports::{RepositoryError, RoomRepository};
use async_trait::async_trait;
use melior_domain::{ChannelId, RoomAccess, User};
use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

/// A [`RoomRepository`] that lives and dies with the process. For tests and for
/// running a single instance without a database; rooms do not survive a restart.
#[derive(Default)]
pub struct InMemoryRoomRepository {
    rooms: Mutex<BTreeMap<ChannelId, RoomAccess>>,
}

impl InMemoryRoomRepository {
    fn lock(&self) -> MutexGuard<'_, BTreeMap<ChannelId, RoomAccess>> {
        self.rooms.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[async_trait]
impl RoomRepository for InMemoryRoomRepository {
    async fn find(&self, channel_id: &ChannelId) -> Result<Option<RoomAccess>, RepositoryError> {
        Ok(self.lock().get(channel_id).cloned())
    }

    async fn insert_if_absent(&self, room: &RoomAccess) -> Result<RoomAccess, RepositoryError> {
        Ok(self.lock().entry(room.channel_id().clone()).or_insert_with(|| room.clone()).clone())
    }

    async fn save(&self, room: &RoomAccess) -> Result<(), RepositoryError> {
        self.lock().insert(room.channel_id().clone(), room.clone());
        Ok(())
    }

    async fn delete(&self, channel_id: &ChannelId) -> Result<(), RepositoryError> {
        self.lock().remove(channel_id);
        Ok(())
    }

    async fn list_for(&self, user: &User) -> Result<Vec<RoomAccess>, RepositoryError> {
        Ok(self
            .lock()
            .values()
            .filter(|room| room.owner_id() == &user.id || room.is_invited(user))
            .cloned()
            .collect())
    }

    async fn list_all(&self) -> Result<Vec<RoomAccess>, RepositoryError> {
        Ok(self.lock().values().cloned().collect())
    }
}
