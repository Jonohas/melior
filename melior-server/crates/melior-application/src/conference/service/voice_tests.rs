//! Capacity, voice states and presence reports.

use super::testing::*;
use super::*;
use crate::conference::{Admission, PresenceContext, PresenceUpdate};
use melior_domain::{ChannelMedia, CommunityId, MediaMode, UserId};

fn claim(muted: bool, camera: bool) -> VoiceState {
    VoiceState { muted, camera, screen: false, speaking: !muted }
}

fn community_admission(h: &Harness, community: &CommunityId, user: &str) -> Admission {
    Admission {
        presence: Some(PresenceContext {
            community_id: community.clone(),
            user_id: UserId::new(format!("sub-{user}")),
        }),
        ..h.admission("general")
    }
}

#[tokio::test]
async fn a_voice_state_reaches_everyone_in_the_room_the_sender_included() {
    let h = Harness::new();
    let alice = h.join_ready("general").await;
    let bob = h.join_ready("general").await;
    let other = h.join_ready("elsewhere").await;

    h.service.set_voice(&alice.id, claim(false, true)).await.unwrap();

    let expected = Signal::VoiceState { session_id: alice.id.clone(), voice: claim(false, true) };
    assert_eq!(alice.sink.take(), vec![expected.clone()]);
    assert_eq!(bob.sink.take(), vec![expected]);
    assert!(other.sink.take().is_empty());
}

#[tokio::test]
async fn an_unchanged_or_corrected_voice_state_is_sent_only_as_accepted() {
    let h = Harness::new();
    let alice = h.join_ready("general").await;

    h.service.set_voice(&alice.id, VoiceState::default()).await.unwrap();
    assert!(alice.sink.take().is_empty(), "nothing changed");

    let muted_but_speaking = VoiceState { muted: true, speaking: true, ..VoiceState::default() };
    h.service.set_voice(&alice.id, muted_but_speaking).await.unwrap();
    let accepted = VoiceState { muted: true, ..VoiceState::default() };
    assert_eq!(
        alice.sink.take(),
        vec![Signal::VoiceState { session_id: alice.id.clone(), voice: accepted }]
    );
}

#[tokio::test]
async fn a_screen_claimed_before_its_track_arrives_is_announced_when_the_track_is_published() {
    let h = Harness::new();
    let alice = h.join_ready("general").await;
    let bob = h.join_ready("general").await;
    h.publish(&alice).await;
    bob.sink.take();

    let sharing = VoiceState { camera: true, screen: true, ..VoiceState::default() };
    h.service.set_voice(&alice.id, sharing).await.unwrap();
    let camera_only = VoiceState { screen: false, ..sharing };
    let announced = |voice| Signal::VoiceState { session_id: alice.id.clone(), voice };
    assert!(bob.sink.take().contains(&announced(camera_only)));

    h.publish(&alice).await;

    assert!(bob.sink.take().contains(&announced(sharing)), "the screen shows once it arrives");
}

#[tokio::test]
async fn voice_states_over_ten_a_second_are_dropped() {
    let h = Harness::new();
    let alice = h.join_ready("general").await;

    for n in 0..15 {
        h.service.set_voice(&alice.id, claim(n % 2 == 0, false)).await.unwrap();
    }

    assert_eq!(alice.sink.take().len(), 10);
}

#[tokio::test]
async fn presence_follows_join_voice_and_leave_in_a_community() {
    let h = Harness::new();
    let community = CommunityId::generate();
    let sink = Arc::new(RecordingSink::default());
    let alice = h
        .service
        .join(community_admission(&h, &community, "alice"), &member("alice"), sink)
        .await
        .unwrap();
    h.service.set_voice(&alice, claim(false, true)).await.unwrap();
    h.service.refresh_presence(&alice).await.unwrap();
    h.service.leave(&alice).await;

    let reports = h.presence.take();
    let [
        PresenceUpdate::Joined { entry: joined, .. },
        PresenceUpdate::Updated { entry: updated, .. },
        PresenceUpdate::Refreshed { entry: refreshed, .. },
        PresenceUpdate::Left { session_id, .. },
    ] = reports.as_slice()
    else {
        panic!("unexpected presence updates {reports:?}");
    };
    assert_eq!(joined.voice, VoiceState::default());
    assert_eq!((joined.username.as_str(), joined.user_id.as_str()), ("alice", "sub-alice"));
    assert_eq!(updated.voice, claim(false, true));
    assert_eq!(refreshed.voice, claim(false, true));
    assert_eq!(*session_id, alice);
    assert!(reports.iter().all(|update| *update.community_id() == community));
}

#[tokio::test]
async fn a_room_outside_a_community_reports_no_presence() {
    let h = Harness::new();
    let alice = h.join_ready("general").await;
    h.service.leave(&alice.id).await;
    assert!(h.presence.take().is_empty());
}

#[tokio::test]
async fn a_full_channel_refuses_the_next_join() {
    let h = Harness::new();
    let media = ChannelMedia { mode: MediaMode::Server, user_limit: Some(2) };
    let plan = RoomPlan::resolve(media, h.service.mesh());
    let admission = Admission { plan, ..h.admission("small") };
    let channel = admission.channel_id.clone();
    for name in ["alice", "bob"] {
        assert!(!h.service.is_full(&channel, plan).await);
        let sink = Arc::new(RecordingSink::default());
        h.service.join(admission.clone(), &member(name), sink).await.unwrap();
    }

    assert!(h.service.is_full(&channel, plan).await);
    let refused =
        h.service.join(admission, &member("carol"), Arc::new(RecordingSink::default())).await;
    assert!(matches!(refused, Err(ConferenceError::Domain(DomainError::ChannelFull))));
}

#[tokio::test]
async fn an_unknown_session_has_no_voice() {
    let h = Harness::new();
    let stranger = SessionId::generate();
    assert!(matches!(
        h.service.set_voice(&stranger, VoiceState::default()).await,
        Err(ConferenceError::UnknownSession)
    ));
    assert!(h.service.refresh_presence(&stranger).await.is_err());
}
