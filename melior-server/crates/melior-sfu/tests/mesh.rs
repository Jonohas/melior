//! End-to-end mesh tests: real WebRTC clients join rooms on a server with mesh on,
//! build direct connections through the relayed `peer_*` messages, and are moved
//! between the direct and the forwarded path.

mod common;
mod mesh_support;

use common::TestServer;
use melior_domain::{MeshPolicy, SessionId};
use mesh_support::{Event, MeshClient, direct_pair};
use serde_json::json;

/// Mesh up to two participants: a third moves the room to the SFU.
async fn mesh_server() -> TestServer {
    TestServer::start_with_mesh(MeshPolicy::new(2)).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_pair_streams_directly_and_pauses_its_uplinks() {
    let server = mesh_server().await;
    // Both play the peer's media over the direct connection (and, until they
    // report, over the SFU), report, and are told to pause the SFU uplink.
    let (alice, bob) = direct_pair(&server, "mesh").await;
    alice.hang_up();
    bob.hang_up();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_room_growing_past_the_limit_moves_the_pair_back_to_the_sfu() {
    let server = mesh_server().await;
    let (mut alice, mut bob) = direct_pair(&server, "mesh").await;
    let (a, b) = (alice.session_id.clone(), bob.session_id.clone());

    let carol = MeshClient::join(&server, "mesh", "carol").await;

    let epoch = alice.expect_route(&b, "forwarded").await;
    assert_eq!(bob.expect_route(&a, "forwarded").await, epoch);
    // The SFU path carries the pair's media again before either side gives up the
    // direct one.
    alice.expect_sfu_media(&b).await;
    bob.expect_sfu_media(&a).await;
    alice.route_live(&b, epoch);
    bob.route_live(&a, epoch);
    alice.expect_link_close(&b).await;
    bob.expect_link_close(&a).await;
    // Each of them also sends to the newcomer, so its uplink resumes.
    alice.expect_uplink(true).await;
    bob.expect_uplink(true).await;
    carol.hang_up();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_direct_link_that_fails_returns_the_pair_to_the_sfu() {
    let server = mesh_server().await;
    let (mut alice, mut bob) = direct_pair(&server, "mesh").await;
    let (a, b) = (alice.session_id.clone(), bob.session_id.clone());

    alice.link_failed(&b);

    let epoch = alice.expect_route(&b, "forwarded").await;
    assert_eq!(bob.expect_route(&a, "forwarded").await, epoch);
    alice.expect_sfu_media(&b).await;
    bob.expect_sfu_media(&a).await;
    alice.expect_uplink(true).await;
    bob.expect_uplink(true).await;
    alice.route_live(&b, epoch);
    bob.route_live(&a, epoch);
    alice.expect_link_close(&b).await;
    bob.expect_link_close(&a).await;
}

fn relays(events: &[Event]) -> usize {
    events.iter().filter(|event| matches!(event, Event::PeerSignal { .. })).count()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn relays_outside_the_room_or_over_the_limits_are_dropped_silently() {
    let server = mesh_server().await;
    let (mut alice, mut bob) = direct_pair(&server, "mesh").await;
    let mut erin = MeshClient::join(&server, "elsewhere", "erin").await;
    let (b, e) = (bob.session_id.clone(), erin.session_id.clone());
    // The pair's own negotiation is over; count what is relayed from here on.
    let (bob_before, erin_before) = (relays(&bob.unclaimed()), relays(&erin.unclaimed()));

    let ice = json!({ "candidate": "candidate:1 1 udp 1 127.0.0.1 9 typ host", "sdpMid": "0" });
    let unknown = SessionId::generate().to_string();
    for to in [e.as_str(), unknown.as_str(), "nobody"] {
        alice.send(&json!({ "channel": "peer_offer", "payload": { "to": to, "sdp": "v=0" } }));
        alice.send(&json!({ "channel": "peer_answer", "payload": { "to": to, "sdp": "v=0" } }));
        alice.send(&json!({ "channel": "peer_ice", "payload": { "to": to, "candidate": ice } }));
    }
    // Over the limits, to a peer of the same room.
    let huge_candidate = json!({ "candidate": "a".repeat(1025), "sdpMid": "0" });
    let huge_sdp = "a".repeat(64 * 1024 + 1);
    alice.send(&json!({ "channel": "peer_offer", "payload": { "to": b, "sdp": huge_sdp } }));
    alice.send(
        &json!({ "channel": "peer_ice", "payload": { "to": b, "candidate": huge_candidate } }),
    );

    // The server handles one socket's messages in order, so the echo returns after
    // all of the above was processed; a relay would already be queued at its target.
    alice.sync().await;
    bob.sync().await;
    erin.sync().await;

    assert_eq!(relays(&erin.unclaimed()), erin_before, "the outsider received a relay");
    assert_eq!(relays(&bob.unclaimed()), bob_before, "a dropped relay reached the room mate");
    assert!(
        !alice.unclaimed().iter().any(|event| matches!(event, Event::ServerError { .. })),
        "the sender must not be told about a dropped relay"
    );
}
