//! [`VoiceRoster`] on Redis: one key per participant that expires unless it is put
//! again, and one set per community naming its participants. A crashed instance
//! stops refreshing its participants' keys, so they disappear on their own.

use super::RedisConnection;
use super::codec::EntryRecord;
use async_trait::async_trait;
use melior_application::{CommunityStoreError, VoiceEntry, VoiceRoster};
use melior_domain::{CommunityId, SessionId};
use std::sync::Arc;
use std::time::Duration;

/// Stores an entry and names it in its community's set. The set lives as long as
/// its longest-lived entry: its expiry only ever grows.
const PUT: &str = r"
redis.call('PSETEX', KEYS[1], ARGV[1], ARGV[2])
redis.call('SADD', KEYS[2], ARGV[3])
if redis.call('PTTL', KEYS[2]) < tonumber(ARGV[1]) then
    redis.call('PEXPIRE', KEYS[2], ARGV[1])
end
return 1
";

/// [`VoiceRoster`] shared between instances through Redis.
pub struct RedisVoiceRoster {
    connection: Arc<RedisConnection>,
}

impl RedisVoiceRoster {
    #[must_use]
    pub fn new(connection: Arc<RedisConnection>) -> Self {
        Self { connection }
    }

    fn set_key(id: &CommunityId) -> String {
        format!("melior:voice:{id}")
    }

    fn entry_key(id: &CommunityId, session: &str) -> String {
        format!("melior:voice:{id}:{session}")
    }
}

#[async_trait]
impl VoiceRoster for RedisVoiceRoster {
    async fn put(
        &self,
        id: &CommunityId,
        entry: &VoiceEntry,
        ttl: Duration,
    ) -> Result<(), CommunityStoreError> {
        let record = serde_json::to_string(&EntryRecord::from(entry)).map_err(store_error)?;
        let ttl_ms = u64::try_from(ttl.as_millis()).unwrap_or(u64::MAX).max(1);
        let session = entry.session_id.as_str();
        let mut connection = self.connection.get().await.map_err(store_error)?;
        ::redis::Script::new(PUT)
            .key(Self::entry_key(id, session))
            .key(Self::set_key(id))
            .arg(ttl_ms)
            .arg(record)
            .arg(session)
            .invoke_async::<()>(&mut connection)
            .await
            .map_err(store_error)
    }

    async fn remove(
        &self,
        id: &CommunityId,
        session: &SessionId,
    ) -> Result<(), CommunityStoreError> {
        let mut connection = self.connection.get().await.map_err(store_error)?;
        ::redis::pipe()
            .atomic()
            .cmd("DEL")
            .arg(Self::entry_key(id, session.as_str()))
            .ignore()
            .cmd("SREM")
            .arg(Self::set_key(id))
            .arg(session.as_str())
            .ignore()
            .query_async::<()>(&mut connection)
            .await
            .map_err(store_error)
    }

    /// Three round trips whatever the size: the set, the entries, and one removal
    /// of the names whose entries expired.
    async fn list(&self, id: &CommunityId) -> Result<Vec<VoiceEntry>, CommunityStoreError> {
        let mut connection = self.connection.get().await.map_err(store_error)?;
        let sessions: Vec<String> = ::redis::cmd("SMEMBERS")
            .arg(Self::set_key(id))
            .query_async(&mut connection)
            .await
            .map_err(store_error)?;
        if sessions.is_empty() {
            return Ok(Vec::new());
        }
        let keys: Vec<String> = sessions.iter().map(|s| Self::entry_key(id, s)).collect();
        let records: Vec<Option<String>> = ::redis::cmd("MGET")
            .arg(keys)
            .query_async(&mut connection)
            .await
            .map_err(store_error)?;

        let expired: Vec<&String> = sessions
            .iter()
            .zip(&records)
            .filter(|(_, record)| record.is_none())
            .map(|(session, _)| session)
            .collect();
        if !expired.is_empty() {
            ::redis::cmd("SREM")
                .arg(Self::set_key(id))
                .arg(expired)
                .query_async::<()>(&mut connection)
                .await
                .map_err(store_error)?;
        }
        let mut entries: Vec<VoiceEntry> = records
            .into_iter()
            .flatten()
            .filter_map(|json| serde_json::from_str::<EntryRecord>(&json).ok()?.into_entry())
            .collect();
        entries.sort_by(|a, b| (&a.channel_id, &a.session_id).cmp(&(&b.channel_id, &b.session_id)));
        Ok(entries)
    }
}

fn store_error(error: impl std::fmt::Display) -> CommunityStoreError {
    CommunityStoreError(error.to_string())
}
