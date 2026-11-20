//! The mesh use cases against the fake ports.

use super::testing::*;
use super::*;
use melior_domain::{MediaPath, MeshPolicy};
use std::time::Duration;

const CHANNEL: &str = "general";

fn harness(limit: usize) -> Harness {
    Harness::with_mesh(MeshPolicy::new(limit))
}

fn routes(signals: &[Signal]) -> Vec<(SessionId, MediaPath, u64)> {
    signals
        .iter()
        .filter_map(|signal| match signal {
            Signal::Route { peer, path, epoch } => Some((peer.clone(), *path, *epoch)),
            _ => None,
        })
        .collect()
}

fn uplinks(signals: &[Signal]) -> Vec<bool> {
    signals
        .iter()
        .filter_map(|signal| match signal {
            Signal::Uplink { active } => Some(*active),
            _ => None,
        })
        .collect()
}

fn offers(signals: &[Signal]) -> usize {
    signals.iter().filter(|signal| matches!(signal, Signal::Offer { .. })).count()
}

fn closes(signals: &[Signal]) -> Vec<SessionId> {
    signals
        .iter()
        .filter_map(|signal| match signal {
            Signal::LinkClose { peer } => Some(peer.clone()),
            _ => None,
        })
        .collect()
}

/// Two participants that published a track each, with the resulting offers answered
/// and every signal so far discarded. The link is `ToDirect` at epoch 1.
async fn pair_with_tracks(h: &Harness) -> (Client, Client, Track, Track) {
    let a = h.join_ready(CHANNEL).await;
    let b = h.join_ready(CHANNEL).await;
    let ta = h.publish(&a).await;
    let tb = h.publish(&b).await;
    h.service.handle_answer(&a.id, Sdp::new("answer")).await.unwrap();
    h.service.handle_answer(&b.id, Sdp::new("answer")).await.unwrap();
    a.sink.take_all();
    b.sink.take_all();
    (a, b, ta, tb)
}

async fn report_both(h: &Harness, a: &Client, b: &Client, epoch: u64) {
    h.service.route_live(&a.id, &b.id, epoch).await.unwrap();
    h.service.route_live(&b.id, &a.id, epoch).await.unwrap();
}

async fn answer_offers(h: &Harness, clients: &[&Client]) {
    for client in clients {
        h.service.handle_answer(&client.id, Sdp::new("answer")).await.unwrap();
    }
}

#[tokio::test(start_paused = true)]
async fn the_second_join_tells_both_sides_to_go_direct() {
    let h = harness(4);
    let a = h.join_ready(CHANNEL).await;
    let b = h.join_ready_keeping(CHANNEL).await;

    assert_eq!(routes(&a.sink.take_all()), vec![(b.id.clone(), MediaPath::Direct, 1)]);
    assert_eq!(routes(&b.sink.take_all()), vec![(a.id.clone(), MediaPath::Direct, 1)]);
}

#[tokio::test(start_paused = true)]
async fn reaching_direct_unsubscribes_and_renegotiates_each_side_once_and_pauses_uplinks() {
    let h = harness(4);
    let (a, b, _, _) = pair_with_tracks(&h).await;
    assert_eq!(a.media.subscribed().len(), 1);

    report_both(&h, &a, &b, 1).await;

    assert!(a.media.subscribed().is_empty() && b.media.subscribed().is_empty());
    let (for_a, for_b) = (a.sink.take_all(), b.sink.take_all());
    assert_eq!((offers(&for_a), offers(&for_b)), (1, 1));
    assert_eq!((uplinks(&for_a), uplinks(&for_b)), (vec![false], vec![false]));
}

#[tokio::test(start_paused = true)]
async fn growing_past_the_limit_moves_direct_pairs_back_through_the_sfu() {
    let h = harness(2);
    let (a, b, ta, tb) = pair_with_tracks(&h).await;
    report_both(&h, &a, &b, 1).await;
    answer_offers(&h, &[&a, &b]).await;
    a.sink.take_all();
    b.sink.take_all();

    let c = h.join_ready_keeping(CHANNEL).await;

    let (for_a, for_b) = (a.sink.take_all(), b.sink.take_all());
    assert_eq!(routes(&for_a), vec![(b.id.clone(), MediaPath::Forwarded, 3)]);
    assert_eq!(routes(&for_b), vec![(a.id.clone(), MediaPath::Forwarded, 3)]);
    assert_eq!((a.media.subscribed(), b.media.subscribed()), (vec![tb.id], vec![ta.id]));
    assert_eq!((offers(&for_a), offers(&for_b)), (1, 1));
    assert_eq!((uplinks(&for_a), uplinks(&for_b)), (vec![true], vec![true]));
    assert!(routes(&c.sink.take_all()).is_empty(), "the newcomer starts forwarded");
}

#[tokio::test(start_paused = true)]
async fn to_forwarded_closes_the_link_when_both_sides_reported() {
    let h = harness(2);
    let a = h.join_ready(CHANNEL).await;
    let b = h.join_ready(CHANNEL).await;
    h.join_ready(CHANNEL).await; // a-b is ToForwarded at epoch 2
    a.sink.take_all();
    b.sink.take_all();

    report_both(&h, &a, &b, 2).await;

    assert_eq!(closes(&a.sink.take_all()), vec![b.id.clone()]);
    assert_eq!(closes(&b.sink.take_all()), vec![a.id.clone()]);
}

#[tokio::test(start_paused = true)]
async fn to_forwarded_goes_back_to_direct_and_keeps_its_subscriptions() {
    let h = harness(3);
    let (alice, bob, _, _) = pair_with_tracks(&h).await;
    let third = h.join_ready(CHANNEL).await;
    let fourth = h.join_ready(CHANNEL).await; // 4 > 3: forwarded
    alice.sink.take_all();
    let subscribed = alice.media.subscribed();

    h.service.leave(&third.id).await;
    h.service.leave(&fourth.id).await;

    let moves = routes(&alice.sink.take_all());
    let to_direct =
        moves.iter().filter(|(peer, path, _)| *peer == bob.id && *path == MediaPath::Direct);
    assert_eq!(to_direct.count(), 1, "the pair moves back to direct once");
    assert_eq!(alice.media.subscribed(), subscribed, "SFU subscriptions stay until Direct");
}

#[tokio::test(start_paused = true)]
async fn the_timeout_pins_the_room_to_forwarded() {
    let h = harness(4);
    let a = h.join_ready(CHANNEL).await;
    let b = h.join_ready(CHANNEL).await;
    a.sink.take_all();
    b.sink.take_all();

    tokio::time::sleep(Duration::from_secs(9)).await;
    assert!(routes(&a.sink.take_all()).is_empty(), "not before 10 s");

    tokio::time::sleep(Duration::from_secs(2)).await;
    assert_eq!(routes(&a.sink.take_all()), vec![(b.id.clone(), MediaPath::Forwarded, 2)]);
    assert_eq!(routes(&b.sink.take_all()), vec![(a.id.clone(), MediaPath::Forwarded, 2)]);
}

#[tokio::test(start_paused = true)]
async fn a_timeout_for_a_link_that_moved_on_does_nothing() {
    let h = harness(4);
    let a = h.join_ready(CHANNEL).await;
    let b = h.join_ready(CHANNEL).await;
    report_both(&h, &a, &b, 1).await;
    a.sink.take_all();
    b.sink.take_all();

    tokio::time::sleep(Duration::from_secs(30)).await;

    assert!(a.sink.take_all().is_empty() && b.sink.take_all().is_empty());
}

#[tokio::test(start_paused = true)]
async fn a_timeout_after_the_room_is_gone_does_nothing() {
    let h = harness(4);
    let a = h.join_ready(CHANNEL).await;
    let b = h.join_ready(CHANNEL).await;
    h.service.leave(&a.id).await;
    h.service.leave(&b.id).await;

    tokio::time::sleep(Duration::from_secs(30)).await;

    assert_eq!(h.service.room_count(), 0);
}

#[tokio::test(start_paused = true)]
async fn a_failed_direct_link_moves_the_room_to_forwarded() {
    let h = harness(4);
    let (a, b, ta, _) = pair_with_tracks(&h).await;
    report_both(&h, &a, &b, 1).await;
    answer_offers(&h, &[&a, &b]).await;
    a.sink.take_all();
    b.sink.take_all();

    h.service.link_failed(&b.id, &a.id).await.unwrap();

    assert_eq!(routes(&a.sink.take_all()), vec![(b.id.clone(), MediaPath::Forwarded, 3)]);
    assert_eq!(b.media.subscribed(), vec![ta.id], "the SFU path is back");
    assert_eq!(uplinks(&b.sink.take_all()), vec![true]);
}

#[tokio::test(start_paused = true)]
async fn a_link_failure_report_without_a_direct_link_is_ignored() {
    let h = harness(2);
    let a = h.join_ready(CHANNEL).await;
    let b = h.join_ready(CHANNEL).await;
    h.join_ready(CHANNEL).await;
    report_both(&h, &a, &b, 2).await; // a-b is Forwarded now
    a.sink.take_all();
    b.sink.take_all();
    let outsider = h.join_ready("other").await;

    h.service.link_failed(&a.id, &b.id).await.unwrap();
    h.service.link_failed(&a.id, &outsider.id).await.unwrap();
    h.service.link_failed(&a.id, &SessionId::generate()).await.unwrap();

    assert!(a.sink.take_all().is_empty() && b.sink.take_all().is_empty());
    assert!(matches!(
        h.service.link_failed(&SessionId::generate(), &a.id).await,
        Err(ConferenceError::UnknownSession)
    ));
}

#[tokio::test(start_paused = true)]
async fn peer_messages_are_relayed_while_a_direct_link_exists() {
    let h = harness(4);
    let a = h.join_ready(CHANNEL).await;
    let b = h.join_ready(CHANNEL).await;
    a.sink.take_all();
    b.sink.take_all();
    let candidate = IceCandidate { candidate: "c".into(), ..IceCandidate::default() };

    h.service.relay_peer_offer(&a.id, &b.id, Sdp::new("o")).await;
    h.service.relay_peer_answer(&a.id, &b.id, Sdp::new("a")).await;
    h.service.relay_peer_ice(&a.id, &b.id, candidate.clone()).await;

    assert_eq!(
        b.sink.take_all(),
        vec![
            Signal::PeerOffer { from: a.id.clone(), sdp: Sdp::new("o") },
            Signal::PeerAnswer { from: a.id.clone(), sdp: Sdp::new("a") },
            Signal::PeerIce { from: a.id.clone(), candidate },
        ]
    );
    assert!(a.sink.take_all().is_empty());
}

#[tokio::test(start_paused = true)]
async fn peer_messages_without_a_direct_link_are_dropped_without_an_error() {
    let h = harness(2);
    let a = h.join_ready(CHANNEL).await;
    let b = h.join_ready(CHANNEL).await;
    let other_room = h.join_ready("other").await;
    h.join_ready(CHANNEL).await;
    report_both(&h, &a, &b, 2).await; // closed: a-b is Forwarded
    a.sink.take_all();
    b.sink.take_all();
    other_room.sink.take_all();
    let unknown = SessionId::generate();

    h.service.relay_peer_offer(&a.id, &b.id, Sdp::new("late")).await;
    h.service.relay_peer_ice(&a.id, &b.id, IceCandidate::default()).await;
    h.service.relay_peer_offer(&a.id, &other_room.id, Sdp::new("x")).await;
    h.service.relay_peer_offer(&a.id, &unknown, Sdp::new("x")).await;
    h.service.relay_peer_answer(&unknown, &a.id, Sdp::new("x")).await;

    for client in [&a, &b, &other_room] {
        assert!(client.sink.take_all().is_empty(), "nothing relayed, no error sent");
    }
}

#[tokio::test(start_paused = true)]
async fn leaving_closes_the_direct_link_of_the_peer() {
    let h = harness(4);
    let a = h.join_ready(CHANNEL).await;
    let b = h.join_ready(CHANNEL).await;
    a.sink.take_all();
    b.sink.take_all();

    h.service.leave(&a.id).await;

    let signals = b.sink.take_all();
    assert_eq!(closes(&signals), vec![a.id.clone()]);
    assert!(signals.contains(&Signal::PeerLeft(a.id.clone())));
}

#[tokio::test(start_paused = true)]
async fn a_service_with_mesh_off_behaves_as_before() {
    let h = Harness::new();
    let a = h.join_ready_keeping(CHANNEL).await;
    let b = h.join_ready_keeping(CHANNEL).await;
    let track = h.publish(&a).await;
    report_both(&h, &a, &b, 1).await;
    h.service.link_failed(&a.id, &b.id).await.unwrap();
    h.service.relay_peer_offer(&a.id, &b.id, Sdp::new("o")).await;
    tokio::time::sleep(Duration::from_secs(30)).await;

    assert_eq!(b.media.subscribed(), vec![track.id]);
    let mesh_signals = |client: &Client| {
        client
            .sink
            .take_all()
            .into_iter()
            .filter(|signal| {
                matches!(
                    signal,
                    Signal::Route { .. }
                        | Signal::LinkClose { .. }
                        | Signal::Uplink { .. }
                        | Signal::PeerOffer { .. }
                )
            })
            .count()
    };
    assert_eq!((mesh_signals(&a), mesh_signals(&b)), (0, 0));
}
