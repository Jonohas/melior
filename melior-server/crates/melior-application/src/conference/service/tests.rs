use super::testing::*;
use super::*;
use melior_domain::{MediaKind, SyncGroup, TrackSource, UserId, VoiceState};

#[tokio::test]
async fn join_announces_the_session_and_first_offer_is_answered() {
    let h = Harness::new();
    let alice = h.join("general").await;

    assert!(matches!(
        alice.sink.take().as_slice(),
        [Signal::Joined { session_id, .. }] if *session_id == alice.id
    ));

    h.service.handle_offer(&alice.id, Sdp::new("offer")).await.unwrap();
    assert_eq!(alice.sink.take(), vec![Signal::Answer(Sdp::new("answer"))]);
}

#[tokio::test]
async fn an_offer_says_whose_track_each_media_section_carries() {
    let h = Harness::new();
    let alice = h.join_ready("general").await;
    let bob = h.join_ready("general").await;
    let carol = h.join_ready("general").await;

    h.publish(&alice).await;
    h.service.handle_answer(&carol.id, Sdp::new("answer")).await.unwrap();
    carol.sink.take_all();
    h.publish(&bob).await;

    let signals = carol.sink.take_all();
    let [Signal::Offer { tracks, .. }] = signals.as_slice() else {
        panic!("expected one offer, got {signals:?}");
    };
    assert_eq!(
        *tracks,
        [
            TrackOwner {
                mid: "0".into(),
                session_id: alice.id.clone(),
                source: TrackSource::Camera
            },
            TrackOwner { mid: "1".into(), session_id: bob.id.clone(), source: TrackSource::Camera },
        ],
        "the complete list, not only what changed"
    );
}

#[tokio::test]
async fn a_screen_is_offered_with_its_source_and_a_stopped_one_is_retracted() {
    let h = Harness::new();
    let alice = h.join_ready("general").await;
    let bob = h.join_ready("general").await;
    h.publish(&alice).await;
    h.service.handle_answer(&bob.id, Sdp::new("answer")).await.unwrap();
    bob.sink.take_all();

    let screen = h.publish(&alice).await;

    let signals = bob.sink.take_all();
    let [Signal::Offer { tracks, .. }] = signals.as_slice() else {
        panic!("expected one offer, got {signals:?}");
    };
    let sources: Vec<TrackSource> = tracks.iter().map(|owner| owner.source).collect();
    assert_eq!(sources, [TrackSource::Camera, TrackSource::Screen]);

    h.service.handle_answer(&bob.id, Sdp::new("answer")).await.unwrap();
    bob.sink.take_all();
    h.service.track_ended(&alice.id, &screen.id).await;
    let signals = bob.sink.take_all();
    let [Signal::Offer { tracks, .. }] = signals.as_slice() else {
        panic!("expected one offer, got {signals:?}");
    };
    assert_eq!(tracks.len(), 1, "only the camera is left");
    assert_eq!(tracks[0].source, TrackSource::Camera);
}

#[tokio::test]
async fn each_track_is_subscribed_in_its_publishers_sync_group() {
    let h = Harness::new();
    let alice = h.join_ready("general").await;
    let bob = h.join_ready("general").await;
    let camera = h.publish(&alice).await;
    let screen = h.publish(&alice).await;

    let camera_group = SyncGroup::new(alice.id.clone(), TrackSource::Camera);
    let screen_group = SyncGroup::new(alice.id.clone(), TrackSource::Screen);
    let expected = vec![(camera.id, camera_group), (screen.id, screen_group)];
    assert_eq!(*lock(&bob.media.groups), expected, "as published");

    let carol = h.join_ready("general").await;
    assert_eq!(*lock(&carol.media.groups), expected, "when joining later");
}

#[tokio::test]
async fn participants_learn_who_is_in_the_room() {
    let h = Harness::new();
    let alice_sink = Arc::new(RecordingSink::default());
    let alice =
        h.service.join(h.admission("general"), &member("alice"), alice_sink.clone()).await.unwrap();
    alice_sink.take_all();

    let bob_sink = Arc::new(RecordingSink::default());
    let bob =
        h.service.join(h.admission("general"), &member("bob"), bob_sink.clone()).await.unwrap();

    assert!(matches!(
        bob_sink.take_all().as_slice(),
        [Signal::Joined { username, participants, .. }]
            if username == "bob"
                && *participants == [ParticipantInfo { session_id: alice.clone(), username: "alice".into(), voice: VoiceState::default() }]
    ));
    assert_eq!(
        alice_sink.take_all(),
        vec![Signal::PeerJoined(ParticipantInfo {
            session_id: bob,
            username: "bob".into(),
            voice: VoiceState::default()
        })]
    );
}

#[tokio::test]
async fn users_without_a_melior_role_cannot_join() {
    let h = Harness::new();
    let guest = User::new(UserId::new("sub-guest"), "guest", None, []);

    let result =
        h.service.join(h.admission("general"), &guest, Arc::new(RecordingSink::default())).await;

    assert!(matches!(result, Err(ConferenceError::NotAllowed(name)) if name == "guest"));
    assert_eq!(h.service.room_count(), 0);
}

#[tokio::test]
async fn published_track_is_offered_to_the_other_participant_only() {
    let h = Harness::new();
    let alice = h.join_ready("general").await;
    let bob = h.join_ready("general").await;

    let track = h.publish(&alice).await;

    assert_eq!(bob.media.subscribed(), vec![track.id]);
    assert_eq!(bob.sink.take(), vec![offer("offer-1")]);
    assert!(alice.media.subscribed().is_empty());
    assert!(alice.sink.take().is_empty());
}

#[tokio::test]
async fn late_joiner_is_offered_all_existing_tracks_once_connected() {
    let h = Harness::new();
    let alice = h.join_ready("general").await;
    let bob = h.join_ready("general").await;
    let a = h.publish(&alice).await;
    let b = h.publish(&bob).await;

    let carol = h.join("general").await;
    carol.sink.take();
    h.service.handle_offer(&carol.id, Sdp::new("offer")).await.unwrap();

    assert_eq!(
        carol.sink.take(),
        vec![Signal::Answer(Sdp::new("answer"))],
        "nothing is offered while the connection is being established"
    );
    assert!(carol.media.subscribed().is_empty());

    // A track published while carol is still connecting is not lost either.
    let c = h.publish(&alice).await;
    assert!(carol.sink.take().is_empty());

    h.service.connected(&carol.id).await;

    let mut expected = vec![a.id, b.id, c.id];
    expected.sort();
    assert_eq!(carol.media.subscribed(), expected);
    assert_eq!(carol.sink.take(), vec![offer("offer-1")], "a single offer covering every track");

    h.service.connected(&carol.id).await;
    assert!(carol.sink.take().is_empty(), "a reconnect changes nothing");
}

#[tokio::test]
async fn changes_during_an_outstanding_offer_are_batched_into_the_next_one() {
    let h = Harness::new();
    let alice = h.join_ready("general").await;
    let bob = h.join_ready("general").await;

    // A camera, a screen and a microphone: a third video would be refused.
    h.publish(&alice).await;
    h.publish(&alice).await;
    let microphone = Track::new(TrackId::generate(), MediaKind::Audio);
    h.service.track_published(&alice.id, microphone).await;
    assert_eq!(bob.sink.take(), vec![offer("offer-1")], "no overlapping offers");

    h.service.handle_answer(&bob.id, Sdp::new("answer")).await.unwrap();
    assert_eq!(bob.sink.take(), vec![offer("offer-2")]);

    h.service.handle_answer(&bob.id, Sdp::new("answer")).await.unwrap();
    assert!(bob.sink.take().is_empty());
    assert_eq!(bob.media.subscribed().len(), 3);
}

#[tokio::test]
async fn leaving_cleans_up_media_subscriptions_and_the_room() {
    let h = Harness::new();
    let alice = h.join_ready("general").await;
    let bob = h.join_ready("general").await;
    h.publish(&alice).await;
    h.service.handle_answer(&bob.id, Sdp::new("answer")).await.unwrap();
    bob.sink.take();

    h.service.leave(&alice.id).await;

    assert!(*lock(&alice.media.closed));
    assert!(bob.media.subscribed().is_empty());
    assert_eq!(bob.sink.take(), vec![Signal::PeerLeft(alice.id.clone()), offer("offer-2"),]);
    assert_eq!(h.service.room_count(), 1);

    h.service.leave(&alice.id).await; // idempotent
    h.service.leave(&bob.id).await;
    assert_eq!(h.service.room_count(), 0);
}

#[tokio::test]
async fn rooms_are_isolated() {
    let h = Harness::new();
    let alice = h.join_ready("red").await;
    let bob = h.join_ready("blue").await;

    h.publish(&alice).await;

    assert!(bob.media.subscribed().is_empty());
    assert!(bob.sink.take().is_empty());
}

#[tokio::test]
async fn ended_track_is_unsubscribed_and_renegotiated() {
    let h = Harness::new();
    let alice = h.join_ready("general").await;
    let bob = h.join_ready("general").await;
    let track = h.publish(&alice).await;
    h.service.handle_answer(&bob.id, Sdp::new("answer")).await.unwrap();
    bob.sink.take();

    h.service.track_ended(&alice.id, &track.id).await;

    assert!(bob.media.subscribed().is_empty());
    assert_eq!(bob.sink.take(), vec![offer("offer-2")]);
}

#[tokio::test]
async fn protocol_violations_are_rejected() {
    let h = Harness::new();
    let alice = h.join_ready("general").await;
    let bob = h.join_ready("general").await;

    assert!(matches!(
        h.service.handle_answer(&bob.id, Sdp::new("answer")).await,
        Err(ConferenceError::Domain(DomainError::UnexpectedAnswer))
    ));

    h.publish(&alice).await;
    assert!(matches!(
        h.service.handle_offer(&bob.id, Sdp::new("offer")).await,
        Err(ConferenceError::Domain(DomainError::OfferDuringRenegotiation))
    ));

    assert!(matches!(
        h.service.handle_offer(&SessionId::generate(), Sdp::new("offer")).await,
        Err(ConferenceError::UnknownSession)
    ));
}

#[tokio::test]
async fn rejected_answer_does_not_block_later_negotiation() {
    let h = Harness::new();
    let alice = h.join_ready("general").await;
    let bob = h.join_ready("general").await;
    h.publish(&alice).await;
    bob.sink.take();

    *lock(&bob.media.reject_answers) = true;
    assert!(h.service.handle_answer(&bob.id, Sdp::new("garbage")).await.is_err());
    *lock(&bob.media.reject_answers) = false;

    h.publish(&alice).await;
    assert_eq!(bob.sink.take(), vec![offer("offer-2")]);
}
