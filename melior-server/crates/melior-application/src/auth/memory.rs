use crate::{SessionStore, StoreError, StoredSession};
use async_trait::async_trait;
use melior_domain::AuthSessionId;
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

/// A [`SessionStore`] that lives and dies with the process. For tests and for
/// running a single instance without Redis; sessions do not survive a restart.
#[derive(Default)]
pub struct InMemorySessionStore {
    sessions: Mutex<HashMap<AuthSessionId, (StoredSession, Instant)>>,
    refresh_claims: Mutex<HashMap<AuthSessionId, (u64, Instant)>>,
    next_lease: std::sync::atomic::AtomicU64,
}

impl InMemorySessionStore {
    /// The number of sessions that have not expired.
    pub fn len(&self) -> usize {
        let now = Instant::now();
        self.lock().values().filter(|(_, expires_at)| *expires_at > now).count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<AuthSessionId, (StoredSession, Instant)>> {
        self.sessions.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[async_trait]
impl SessionStore for InMemorySessionStore {
    async fn save(
        &self,
        id: &AuthSessionId,
        session: &StoredSession,
        ttl: Duration,
    ) -> Result<(), StoreError> {
        let mut sessions = self.lock();
        let now = Instant::now();
        sessions.retain(|_, (_, expires_at)| *expires_at > now);
        sessions.insert(id.clone(), (session.clone(), now + ttl));
        Ok(())
    }

    async fn load(&self, id: &AuthSessionId) -> Result<Option<StoredSession>, StoreError> {
        Ok(self
            .lock()
            .get(id)
            .filter(|(_, expires_at)| *expires_at > Instant::now())
            .map(|(session, _)| session.clone()))
    }

    async fn delete(&self, id: &AuthSessionId) -> Result<(), StoreError> {
        self.lock().remove(id);
        Ok(())
    }

    async fn claim_refresh(
        &self,
        id: &AuthSessionId,
        ttl: Duration,
    ) -> Result<Option<String>, StoreError> {
        let mut claims = self.refresh_claims.lock().unwrap_or_else(PoisonError::into_inner);
        let now = Instant::now();
        if claims.get(id).is_some_and(|(_, expires_at)| *expires_at > now) {
            return Ok(None);
        }
        let lease = self.next_lease.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        claims.insert(id.clone(), (lease, now + ttl));
        Ok(Some(lease.to_string()))
    }

    async fn release_refresh(&self, id: &AuthSessionId, lease: &str) -> Result<(), StoreError> {
        let mut claims = self.refresh_claims.lock().unwrap_or_else(PoisonError::into_inner);
        if claims.get(id).is_some_and(|(held, _)| held.to_string() == lease) {
            claims.remove(id);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Tokens;
    use melior_domain::{User, UserId};

    fn session() -> StoredSession {
        StoredSession {
            user: User::new(UserId::new("sub"), "alice", None, []),
            tokens: Tokens {
                access_token: "a".into(),
                refresh_token: None,
                id_token: None,
                access_token_expires_at: 0,
            },
        }
    }

    #[tokio::test]
    async fn stores_expires_and_deletes() {
        let store = InMemorySessionStore::default();
        let id = AuthSessionId::generate();

        store.save(&id, &session(), Duration::from_secs(60)).await.unwrap();
        assert_eq!(store.load(&id).await.unwrap(), Some(session()));

        store.save(&id, &session(), Duration::ZERO).await.unwrap();
        assert_eq!(store.load(&id).await.unwrap(), None, "expired");

        store.save(&id, &session(), Duration::from_secs(60)).await.unwrap();
        store.delete(&id).await.unwrap();
        assert!(store.is_empty());
    }

    #[tokio::test]
    async fn a_refresh_claim_is_exclusive_until_released_or_expired() {
        let store = InMemorySessionStore::default();
        let id = AuthSessionId::generate();

        let lease = store.claim_refresh(&id, Duration::from_secs(30)).await.unwrap().unwrap();
        assert_eq!(store.claim_refresh(&id, Duration::from_secs(30)).await.unwrap(), None);
        assert!(
            store
                .claim_refresh(&AuthSessionId::generate(), Duration::from_secs(30))
                .await
                .unwrap()
                .is_some()
        );

        store.release_refresh(&id, "not-the-lease").await.unwrap();
        assert_eq!(store.claim_refresh(&id, Duration::from_secs(30)).await.unwrap(), None);

        store.release_refresh(&id, &lease).await.unwrap();
        assert!(store.claim_refresh(&id, Duration::ZERO).await.unwrap().is_some());
        assert!(
            store.claim_refresh(&id, Duration::from_secs(30)).await.unwrap().is_some(),
            "an expired claim is free"
        );
    }
}
