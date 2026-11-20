//! The community event bus and the voice roster on a real Redis. Needs one, so it
//! is ignored by default:
//!
//! ```sh
//! MELIOR_TEST_REDIS_URL=redis://127.0.0.1:6379/9 \
//!     cargo test -p melior-infrastructure --test redis_communities -- --ignored
//! ```

use melior_application::{CommunityEvent, CommunityEvents, VoiceEntry, VoiceRoster};
use melior_domain::{ChannelId, CommunityId, SessionId, UserId, VoiceState};
use melior_infrastructure::redis::{RedisCommunityEvents, RedisConnection, RedisVoiceRoster};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

fn connection() -> Arc<RedisConnection> {
    let url = std::env::var("MELIOR_TEST_REDIS_URL").expect("MELIOR_TEST_REDIS_URL");
    Arc::new(RedisConnection::new(&url).expect("redis url"))
}

fn entry(channel: &str) -> VoiceEntry {
    VoiceEntry {
        channel_id: ChannelId::parse(channel).unwrap(),
        session_id: SessionId::generate(),
        user_id: UserId::new("sub-alice"),
        username: "alice".into(),
        voice: VoiceState { muted: true, ..VoiceState::default() },
    }
}

#[tokio::test]
#[ignore = "needs MELIOR_TEST_REDIS_URL"]
async fn an_event_published_on_one_instance_reaches_the_subscribers_of_another() {
    let (here, there) =
        (RedisCommunityEvents::new(connection()), RedisCommunityEvents::new(connection()));
    let community = CommunityId::generate();
    let other = CommunityId::generate();
    let mut feed = there.subscribe(&community);
    let mut elsewhere = there.subscribe(&other);
    // The subscriber connects in the background; give it a moment.
    tokio::time::sleep(Duration::from_millis(300)).await;

    let joined = CommunityEvent::VoiceJoined(entry("general"));
    here.publish(&community, joined.clone()).await.unwrap();
    here.publish(&community, CommunityEvent::StructureChanged).await.unwrap();

    let step = Duration::from_secs(5);
    assert_eq!(timeout(step, feed.recv()).await.unwrap().unwrap(), joined);
    assert_eq!(
        timeout(step, feed.recv()).await.unwrap().unwrap(),
        CommunityEvent::StructureChanged
    );
    assert!(elsewhere.try_recv().is_err(), "another community hears nothing");
}

#[tokio::test]
#[ignore = "needs MELIOR_TEST_REDIS_URL"]
async fn the_roster_keeps_entries_until_they_are_removed_or_expire() {
    let roster = RedisVoiceRoster::new(connection());
    let community = CommunityId::generate();
    let (stays, leaves, expires) = (entry("general"), entry("general"), entry("other"));
    let long = Duration::from_secs(60);
    roster.put(&community, &stays, long).await.unwrap();
    roster.put(&community, &leaves, long).await.unwrap();
    roster.put(&community, &expires, Duration::from_millis(100)).await.unwrap();

    roster.remove(&community, &leaves.session_id).await.unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;

    assert_eq!(roster.list(&community).await.unwrap(), vec![stays.clone()]);
    let updated = VoiceEntry { voice: VoiceState::default(), ..stays };
    roster.put(&community, &updated, long).await.unwrap();
    assert_eq!(roster.list(&community).await.unwrap(), vec![updated]);
    assert!(roster.list(&CommunityId::generate()).await.unwrap().is_empty());
}
