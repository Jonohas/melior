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

/// Text that only the messages this test drops carry.
const DROPPED: &str = "dropped-marker";
/// Text of the one valid relay this test sends, to show relaying works at all.
const DELIVERED: &str = "delivered-marker";

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn relays_outside_the_room_or_over_the_limits_are_dropped_silently() {
    let server = mesh_server().await;
    let (mut alice, mut bob) = direct_pair(&server, "mesh").await;
    let mut erin = MeshClient::join(&server, "elsewhere", "erin").await;
    let (b, e) = (bob.session_id.clone(), erin.session_id.clone());

    let ice = |candidate: String| json!({ "candidate": candidate, "sdpMid": "0" });
    let marked = |text: &str| format!("{DROPPED}-{text}");
    let unknown = SessionId::generate().to_string();
    for to in [e.as_str(), unknown.as_str(), "nobody"] {
        let sdp = marked("sdp");
        alice.send(&json!({ "channel": "peer_offer", "payload": { "to": to, "sdp": sdp } }));
        alice.send(&json!({ "channel": "peer_answer", "payload": { "to": to, "sdp": sdp } }));
        let candidate = ice(marked("candidate"));
        alice.send(
            &json!({ "channel": "peer_ice", "payload": { "to": to, "candidate": candidate } }),
        );
    }
    // Over the limits, to a peer of the same room.
    let huge_sdp = format!("{}{}", marked("sdp"), "a".repeat(64 * 1024));
    alice.send(&json!({ "channel": "peer_offer", "payload": { "to": b, "sdp": huge_sdp } }));
    let huge_candidate = ice(format!("{}{}", marked("candidate"), "a".repeat(1024)));
    alice.send(
        &json!({ "channel": "peer_ice", "payload": { "to": b, "candidate": huge_candidate } }),
    );
    let long_field = json!({ "candidate": marked("mid"), "sdpMid": "m".repeat(257) });
    alice.send(&json!({ "channel": "peer_ice", "payload": { "to": b, "candidate": long_field } }));
    let long_fragment =
        json!({ "candidate": marked("ufrag"), "usernameFragment": "u".repeat(257) });
    alice.send(
        &json!({ "channel": "peer_ice", "payload": { "to": b, "candidate": long_fragment } }),
    );
    // A valid one last: when it arrives, everything sent before it has been handled.
    let control = ice(DELIVERED.to_owned());
    alice.send(&json!({ "channel": "peer_ice", "payload": { "to": b, "candidate": control } }));

    alice.sync().await;
    bob.expect(
        DELIVERED,
        |event| matches!(event, Event::PeerSignal { body, .. } if body == DELIVERED),
    )
    .await;
    erin.sync().await;

    assert!(!bob.saw_relay_containing(DROPPED), "a dropped relay reached the room mate");
    assert!(!erin.saw_relay_containing(DROPPED), "the outsider received a relay");
    assert!(
        !alice.unclaimed().iter().any(|event| matches!(event, Event::ServerError { .. })),
        "the sender must not be told about a dropped relay"
    );
}
