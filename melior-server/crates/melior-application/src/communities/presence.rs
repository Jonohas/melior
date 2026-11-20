//! The presence sink the conference reports to: a queue drained by one task, which
//! keeps the voice roster and tells the community. Reports are handled in the
//! order they were made; a room never waits on the roster or the event bus.

use super::ports::{CommunityEvent, CommunityEvents, VoiceRoster};
use crate::conference::{PresenceSink, PresenceUpdate};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::warn;

/// A [`PresenceSink`] backed by a [`VoiceRoster`] and [`CommunityEvents`].
pub struct PresenceQueue {
    queue: mpsc::UnboundedSender<PresenceUpdate>,
}

impl PresenceQueue {
    /// Starts the task that drains the queue. Roster entries live `ttl` unless
    /// they are refreshed. Must be called within a tokio runtime.
    #[must_use]
    pub fn start(
        roster: Arc<dyn VoiceRoster>,
        events: Arc<dyn CommunityEvents>,
        ttl: Duration,
    ) -> Arc<Self> {
        let (queue, mut updates) = mpsc::unbounded_channel::<PresenceUpdate>();
        tokio::spawn(async move {
            while let Some(update) = updates.recv().await {
                apply(roster.as_ref(), events.as_ref(), ttl, update).await;
            }
        });
        Arc::new(Self { queue })
    }
}

impl PresenceSink for PresenceQueue {
    fn report(&self, update: PresenceUpdate) {
        // Fails only once the draining task is gone, at shutdown.
        let _ = self.queue.send(update);
    }
}

/// Carries out one report. A failing roster or bus is logged; the next report,
/// or the next refresh, puts things right.
async fn apply(
    roster: &dyn VoiceRoster,
    events: &dyn CommunityEvents,
    ttl: Duration,
    update: PresenceUpdate,
) {
    let (community_id, stored, event) = match update {
        PresenceUpdate::Joined { community_id, entry } => {
            let stored = roster.put(&community_id, &entry, ttl).await;
            (community_id, stored, Some(CommunityEvent::VoiceJoined(entry)))
        }
        PresenceUpdate::Updated { community_id, entry } => {
            let stored = roster.put(&community_id, &entry, ttl).await;
            (community_id, stored, Some(CommunityEvent::VoiceUpdated(entry)))
        }
        PresenceUpdate::Refreshed { community_id, entry } => {
            let stored = roster.put(&community_id, &entry, ttl).await;
            (community_id, stored, None)
        }
        PresenceUpdate::Left { community_id, channel_id, session_id } => {
            let stored = roster.remove(&community_id, &session_id).await;
            (community_id, stored, Some(CommunityEvent::VoiceLeft { channel_id, session_id }))
        }
    };
    if let Err(error) = stored {
        warn!(%error, community = %community_id, "voice roster not updated");
    }
    let Some(event) = event else {
        return;
    };
    if let Err(error) = events.publish(&community_id, event).await {
        warn!(%error, community = %community_id, "voice presence not published");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::communities::{InMemoryCommunityEvents, InMemoryVoiceRoster};
    use crate::conference::VoiceEntry;
    use melior_domain::{ChannelId, CommunityId, SessionId, UserId, VoiceState};

    fn entry(voice: VoiceState) -> VoiceEntry {
        VoiceEntry {
            channel_id: ChannelId::parse("general").unwrap(),
            session_id: SessionId::generate(),
            user_id: UserId::new("sub-alice"),
            username: "alice".into(),
            voice,
        }
    }

    #[tokio::test]
    async fn reports_keep_the_roster_and_reach_the_community_in_order() {
        let roster = Arc::new(InMemoryVoiceRoster::default());
        let events = Arc::new(InMemoryCommunityEvents::default());
        let community = CommunityId::generate();
        let mut feed = events.subscribe(&community);
        let queue = PresenceQueue::start(roster.clone(), events.clone(), Duration::from_secs(60));
        let joined = entry(VoiceState::default());
        let updated = VoiceEntry {
            voice: VoiceState { muted: true, ..VoiceState::default() },
            ..joined.clone()
        };

        queue.report(PresenceUpdate::joined(community.clone(), joined.clone()));
        queue.report(PresenceUpdate::refreshed(community.clone(), joined.clone()));
        queue.report(PresenceUpdate::updated(community.clone(), updated.clone()));

        assert_eq!(feed.recv().await.unwrap(), CommunityEvent::VoiceJoined(joined.clone()));
        assert_eq!(feed.recv().await.unwrap(), CommunityEvent::VoiceUpdated(updated.clone()));
        assert_eq!(roster.list(&community).await.unwrap(), vec![updated]);

        queue.report(PresenceUpdate::Left {
            community_id: community.clone(),
            channel_id: joined.channel_id.clone(),
            session_id: joined.session_id.clone(),
        });
        assert_eq!(
            feed.recv().await.unwrap(),
            CommunityEvent::VoiceLeft {
                channel_id: joined.channel_id,
                session_id: joined.session_id
            }
        );
        assert!(roster.list(&community).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn an_entry_that_is_not_refreshed_expires() {
        let roster = InMemoryVoiceRoster::default();
        let community = CommunityId::generate();
        roster.put(&community, &entry(VoiceState::default()), Duration::ZERO).await.unwrap();
        assert!(roster.list(&community).await.unwrap().is_empty());
    }
}
