use super::RedisConnection;
use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use melior_application::{SessionStore, StoreError, StoredSession, Tokens};
use melior_domain::{AuthSessionId, User, UserId};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;

/// Deletes the refresh claim only if it is still the caller's.
const RELEASE: &str = r"
if redis.call('GET', KEYS[1]) == ARGV[1] then
    return redis.call('DEL', KEYS[1])
end
return 0
";

/// A [`SessionStore`] on Redis: one key per session, expired by Redis itself.
pub struct RedisSessionStore {
    connection: Arc<RedisConnection>,
}

impl RedisSessionStore {
    #[must_use]
    pub fn new(connection: Arc<RedisConnection>) -> Self {
        Self { connection }
    }

    fn key(id: &AuthSessionId) -> String {
        format!("session:default:{}", id.as_str())
    }

    fn refresh_key(id: &AuthSessionId) -> String {
        format!("session-refresh:default:{}", id.as_str())
    }
}

#[async_trait]
impl SessionStore for RedisSessionStore {
    async fn save(
        &self,
        id: &AuthSessionId,
        session: &StoredSession,
        ttl: Duration,
    ) -> Result<(), StoreError> {
        let record = serde_json::to_string(&Record::from(session)).map_err(store_error)?;
        let ttl_ms = u64::try_from(ttl.as_millis()).unwrap_or(u64::MAX).max(1);
        let mut connection = self.connection.get().await.map_err(store_error)?;
        ::redis::cmd("PSETEX")
            .arg(Self::key(id))
            .arg(ttl_ms)
            .arg(record)
            .query_async::<()>(&mut connection)
            .await
            .map_err(store_error)
    }

    async fn load(&self, id: &AuthSessionId) -> Result<Option<StoredSession>, StoreError> {
        let mut connection = self.connection.get().await.map_err(store_error)?;
        let record: Option<String> = ::redis::cmd("GET")
            .arg(Self::key(id))
            .query_async(&mut connection)
            .await
            .map_err(store_error)?;
        record
            .map(|json| {
                serde_json::from_str::<Record>(&json).map(StoredSession::from).map_err(store_error)
            })
            .transpose()
    }

    async fn delete(&self, id: &AuthSessionId) -> Result<(), StoreError> {
        let mut connection = self.connection.get().await.map_err(store_error)?;
        ::redis::cmd("DEL")
            .arg(Self::key(id))
            .query_async::<()>(&mut connection)
            .await
            .map_err(store_error)
    }

    async fn claim_refresh(
        &self,
        id: &AuthSessionId,
        ttl: Duration,
    ) -> Result<Option<String>, StoreError> {
        let mut lease = [0u8; 16];
        rand::rngs::OsRng.fill_bytes(&mut lease);
        let lease = URL_SAFE_NO_PAD.encode(lease);
        let ttl_ms = u64::try_from(ttl.as_millis()).unwrap_or(u64::MAX).max(1);

        let mut connection = self.connection.get().await.map_err(store_error)?;
        let claimed: Option<String> = ::redis::cmd("SET")
            .arg(Self::refresh_key(id))
            .arg(&lease)
            .arg("NX")
            .arg("PX")
            .arg(ttl_ms)
            .query_async(&mut connection)
            .await
            .map_err(store_error)?;
        Ok(claimed.map(|_| lease))
    }

    async fn release_refresh(&self, id: &AuthSessionId, lease: &str) -> Result<(), StoreError> {
        let mut connection = self.connection.get().await.map_err(store_error)?;
        ::redis::Script::new(RELEASE)
            .key(Self::refresh_key(id))
            .arg(lease)
            .invoke_async::<i64>(&mut connection)
            .await
            .map_err(store_error)?;
        Ok(())
    }
}

fn store_error(error: impl std::fmt::Display) -> StoreError {
    StoreError(error.to_string())
}

/// The stored JSON document.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Record {
    subject: String,
    username: String,
    email: Option<String>,
    roles: Vec<String>,
    access_token: String,
    refresh_token: Option<String>,
    id_token: Option<String>,
    access_token_expires_at: u64,
}

impl From<&StoredSession> for Record {
    fn from(session: &StoredSession) -> Self {
        Self {
            subject: session.user.id.as_str().to_owned(),
            username: session.user.username.clone(),
            email: session.user.email.clone(),
            roles: session.user.roles().to_vec(),
            access_token: session.tokens.access_token.clone(),
            refresh_token: session.tokens.refresh_token.clone(),
            id_token: session.tokens.id_token.clone(),
            access_token_expires_at: session.tokens.access_token_expires_at,
        }
    }
}

impl From<Record> for StoredSession {
    fn from(record: Record) -> Self {
        Self {
            user: User::new(
                UserId::new(record.subject),
                record.username,
                record.email,
                record.roles,
            ),
            tokens: Tokens {
                access_token: record.access_token,
                refresh_token: record.refresh_token,
                id_token: record.id_token,
                access_token_expires_at: record.access_token_expires_at,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_round_trips_a_session() {
        let session = StoredSession {
            user: User::new(
                UserId::new("sub-1"),
                "alice",
                Some("alice@example.test".into()),
                ["member".to_owned()],
            ),
            tokens: Tokens {
                access_token: "access".into(),
                refresh_token: Some("refresh".into()),
                id_token: Some("id".into()),
                access_token_expires_at: 1_800_000_000,
            },
        };

        let json = serde_json::to_string(&Record::from(&session)).unwrap();
        assert!(json.contains("\"accessTokenExpiresAt\":1800000000"));
        let back: StoredSession = serde_json::from_str::<Record>(&json).unwrap().into();
        assert_eq!(back, session);
    }

    #[test]
    fn keys_are_namespaced() {
        let id = AuthSessionId::generate();
        assert_eq!(RedisSessionStore::key(&id), format!("session:default:{}", id.as_str()));
    }
}
