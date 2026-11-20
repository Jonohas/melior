//! [`CommunityEvents`] on Redis pub/sub: `PUBLISH melior:community:<id>` with the
//! event as JSON, and one subscriber connection per instance that fans what it
//! receives out to the local subscribers.

use super::{RedisConnection, codec};
use async_trait::async_trait;
use futures_util::StreamExt;
use melior_application::{CommunityEvent, CommunityEvents, EventsError, InMemoryCommunityEvents};
use melior_domain::CommunityId;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::broadcast;
use tracing::{debug, info, warn};

const CHANNEL_PREFIX: &str = "melior:community:";
/// How long the subscriber waits before it connects again after losing Redis.
const RECONNECT_DELAY: Duration = Duration::from_secs(1);

/// [`CommunityEvents`] shared between instances through Redis.
pub struct RedisCommunityEvents {
    connection: Arc<RedisConnection>,
    local: Arc<InMemoryCommunityEvents>,
    listening: AtomicBool,
}

impl RedisCommunityEvents {
    /// Connects nothing yet: the subscriber starts with the first subscription.
    #[must_use]
    pub fn new(connection: Arc<RedisConnection>) -> Self {
        Self {
            connection,
            local: Arc::new(InMemoryCommunityEvents::default()),
            listening: AtomicBool::new(false),
        }
    }

    fn channel(id: &CommunityId) -> String {
        format!("{CHANNEL_PREFIX}{id}")
    }

    /// Starts the one subscriber of this instance, once. It reconnects for as long
    /// as the process runs.
    fn listen(&self) {
        if self.listening.swap(true, Ordering::SeqCst) {
            return;
        }
        let client = self.connection.client().clone();
        let local = self.local.clone();
        tokio::spawn(async move {
            loop {
                if let Err(error) = receive(&client, &local).await {
                    warn!(%error, "community event subscription lost; reconnecting");
                }
                tokio::time::sleep(RECONNECT_DELAY).await;
            }
        });
    }
}

/// Receives events until the subscription ends.
async fn receive(
    client: &::redis::Client,
    local: &InMemoryCommunityEvents,
) -> Result<(), ::redis::RedisError> {
    let mut pubsub = client.get_async_pubsub().await?;
    pubsub.psubscribe(format!("{CHANNEL_PREFIX}*")).await?;
    info!("subscribed to community events");
    let mut messages = pubsub.on_message();
    while let Some(message) = messages.next().await {
        let Some(id) =
            message.get_channel_name().strip_prefix(CHANNEL_PREFIX).and_then(CommunityId::parse)
        else {
            continue;
        };
        let event = message.get_payload::<String>().ok().as_deref().and_then(codec::decode);
        let Some(event) = event else {
            debug!(community = %id, "undecodable community event dropped");
            continue;
        };
        local.deliver(&id, event);
    }
    Ok(())
}

#[async_trait]
impl CommunityEvents for RedisCommunityEvents {
    async fn publish(&self, id: &CommunityId, event: CommunityEvent) -> Result<(), EventsError> {
        let payload = codec::encode(&event).map_err(events_error)?;
        let mut connection = self.connection.get().await.map_err(events_error)?;
        ::redis::cmd("PUBLISH")
            .arg(Self::channel(id))
            .arg(payload)
            .query_async::<i64>(&mut connection)
            .await
            .map_err(events_error)?;
        Ok(())
    }

    fn subscribe(&self, id: &CommunityId) -> broadcast::Receiver<CommunityEvent> {
        self.listen();
        self.local.subscribe(id)
    }
}

fn events_error(error: impl std::fmt::Display) -> EventsError {
    EventsError(error.to_string())
}
