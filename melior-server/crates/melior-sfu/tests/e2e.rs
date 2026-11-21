//! End-to-end test: logged-in WebRTC clients talk to the real server over WebSockets
//! and exchange RTP through it.

mod common;

use common::{STEP, TestServer, new_peer_connection, next_json};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use webrtc::api::media_engine::MIME_TYPE_VP8;
use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::rtp::header::Header;
use webrtc::rtp::packet::Packet;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::track::track_local::track_local_static_rtp::TrackLocalStaticRTP;
use webrtc::track::track_local::{TrackLocal, TrackLocalWriter};

/// Something a test client observed.
#[derive(Debug)]
enum Event {
    /// RTP arrived on a track belonging to the stream with this id.
    Media {
        stream_id: String,
    },
    PeerLeft {
        session_id: String,
    },
    ServerError {
        message: String,
    },
}

/// A participant that behaves like the browser client: it publishes one VP8 track,
/// makes the first offer, and answers the server's renegotiation offers.
struct Client {
    session_id: String,
    events: mpsc::UnboundedReceiver<Event>,
    connection: Arc<RTCPeerConnection>,
    /// Dropping this closes the WebSocket, like closing the browser tab.
    hangup: mpsc::UnboundedSender<()>,
    /// How many media sections the server's latest offer had.
    media_sections: Arc<AtomicUsize>,
}

/// What a server offer says: how many media sections it has, and which of them
/// carry whose track (`(mid, session id)`, from the `tracks` list next to the SDP).
fn offered_sections(payload: &Value) -> (usize, Vec<(String, String)>) {
    let total = payload["sdp"].as_str().unwrap().matches("\r\nm=").count();
    let carrying = payload["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|owner| {
            (
                owner["mid"].as_str().unwrap().to_owned(),
                owner["session_id"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    (total, carrying)
}

impl Client {
    /// Logs `username` in as a member and joins the channel.
    // One linear script of what a browser client does; splitting it would only
    // scatter the shared connection, channels and tasks across helpers.
    #[allow(clippy::too_many_lines)]
    async fn join(server: &TestServer, channel: &str, username: &str) -> Self {
        server.voice_channel(channel).await;
        let cookie = server.login(username, &["member"]).await;
        let request = server.ws_request(Some(channel), Some(&cookie));
        let (socket, _) = tokio_tungstenite::connect_async(request).await.expect("connect");
        let (mut ws_tx, mut ws_rx) = socket.split();

        // The server speaks first.
        let hello = next_json(&mut ws_rx).await;
        assert_eq!(hello["channel"], "session");
        assert_eq!(hello["payload"]["channel_id"], channel);
        assert_eq!(hello["payload"]["username"], username);
        let session_id = hello["payload"]["session_id"].as_str().unwrap().to_owned();

        let connection = Arc::new(new_peer_connection().await);
        let (events_tx, events) = mpsc::unbounded_channel();
        let (out_tx, mut out_rx) = mpsc::unbounded_channel::<Value>();
        let (hangup, mut hangup_rx) = mpsc::unbounded_channel::<()>();

        // Publish a track and keep sending RTP on it.
        let track = Arc::new(TrackLocalStaticRTP::new(
            RTCRtpCodecCapability { mime_type: MIME_TYPE_VP8.to_owned(), ..Default::default() },
            "camera".to_owned(),
            "local".to_owned(),
        ));
        connection.add_track(track.clone() as Arc<dyn TrackLocal + Send + Sync>).await.unwrap();
        tokio::spawn(async move {
            let mut sequence_number = 0u16;
            loop {
                let packet = Packet {
                    header: Header {
                        version: 2,
                        sequence_number,
                        timestamp: u32::from(sequence_number) * 3000,
                        ..Default::default()
                    },
                    payload: vec![0x10, 0x00, 0x00, 0x00].into(),
                };
                if track.write_rtp(&packet).await.is_err() {
                    break;
                }
                sequence_number = sequence_number.wrapping_add(1);
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        });

        let ice_out = out_tx.clone();
        connection.on_ice_candidate(Box::new(move |candidate| {
            if let Some(candidate) = candidate {
                let init = candidate.to_json().unwrap();
                let _ = ice_out.send(json!({ "channel": "ice", "payload": init }));
            }
            Box::pin(async {})
        }));

        // When RTP last arrived, per media section. The server reuses a section for
        // another participant's track once its first one is gone, and only the
        // `tracks` list of the next offer says so; so "media from X" is judged by
        // what an offer announces and whether packets then arrive on that section.
        let arrivals: Arc<Mutex<HashMap<String, Instant>>> = Arc::default();
        let on_track_arrivals = arrivals.clone();
        connection.on_track(Box::new(move |remote, _, transceiver| {
            let arrivals = on_track_arrivals.clone();
            let mid = transceiver.mid().map(|mid| mid.to_string()).unwrap_or_default();
            tokio::spawn(async move {
                while remote.read_rtp().await.is_ok() {
                    arrivals.lock().unwrap().insert(mid.clone(), Instant::now());
                }
            });
            Box::pin(async {})
        }));
        let media_sections = Arc::new(AtomicUsize::new(0));
        let section_count = media_sections.clone();

        let offer = connection.create_offer(None).await.unwrap();
        // Queue the offer before gathering starts: no candidate may overtake it.
        out_tx.send(json!({ "channel": "sdp_offer", "payload": offer })).unwrap();
        connection.set_local_description(offer).await.unwrap();

        // The signalling loop: one message at a time, like the browser client.
        let pc = connection.clone();
        tokio::spawn(async move {
            let mut early_candidates: Vec<RTCIceCandidateInit> = Vec::new();
            let mut announced: HashSet<(String, String)> = HashSet::new();
            loop {
                tokio::select! {
                    outgoing = out_rx.recv() => {
                        let Some(outgoing) = outgoing else { break };
                        if ws_tx.send(Message::Text(outgoing.to_string().into())).await.is_err() {
                            break;
                        }
                    }
                    _ = hangup_rx.recv() => {
                        let _ = ws_tx.close().await;
                        break;
                    }
                    incoming = ws_rx.next() => {
                        let Some(Ok(Message::Text(text))) = incoming else { break };
                        let message: Value = serde_json::from_str(text.as_str()).unwrap();
                        let payload = message["payload"].clone();
                        match message["channel"].as_str().unwrap() {
                            "sdp_answer" | "sdp_offer" => {
                                let offered = (message["channel"] == "sdp_offer")
                                    .then(|| offered_sections(&payload));
                                let description: RTCSessionDescription =
                                    serde_json::from_value(payload).unwrap();
                                pc.set_remote_description(description).await.unwrap();
                                if let Some((total, sending)) = offered {
                                    section_count.store(total, Ordering::SeqCst);
                                    announced.retain(|section| sending.contains(section));
                                    for section in sending {
                                        if announced.insert(section.clone()) {
                                            report_when_flowing(section, arrivals.clone(), events_tx.clone());
                                        }
                                    }
                                }
                                for candidate in early_candidates.drain(..) {
                                    pc.add_ice_candidate(candidate).await.unwrap();
                                }
                                if message["channel"] == "sdp_offer" {
                                    let answer = pc.create_answer(None).await.unwrap();
                                    pc.set_local_description(answer.clone()).await.unwrap();
                                    let _ = out_tx.send(json!({ "channel": "sdp_answer", "payload": answer }));
                                }
                            }
                            "ice" => {
                                let candidate: RTCIceCandidateInit =
                                    serde_json::from_value(payload).unwrap();
                                if pc.remote_description().await.is_some() {
                                    pc.add_ice_candidate(candidate).await.unwrap();
                                } else {
                                    early_candidates.push(candidate);
                                }
                            }
                            "peer_joined" => {}
                            "peer_left" => {
                                let _ = events_tx.send(Event::PeerLeft {
                                    session_id: payload["session_id"].as_str().unwrap().to_owned(),
                                });
                            }
                            "error" => {
                                let _ = events_tx.send(Event::ServerError {
                                    message: payload["message"].as_str().unwrap().to_owned(),
                                });
                            }
                            other => panic!("unexpected channel {other}"),
                        }
                    }
                }
            }
        });

        Self { session_id, events, connection, hangup, media_sections }
    }

    fn media_sections(&self) -> usize {
        self.media_sections.load(Ordering::SeqCst)
    }

    /// Waits until media has arrived from exactly the given participants.
    async fn expect_media_from(&mut self, expected: &[&str]) {
        let mut missing: HashSet<String> =
            expected.iter().map(std::string::ToString::to_string).collect();
        while !missing.is_empty() {
            match self.next_event().await {
                Event::Media { stream_id } => {
                    assert!(
                        missing.remove(&stream_id),
                        "media from unexpected stream {stream_id}; still waiting for {missing:?}"
                    );
                }
                Event::ServerError { message } => panic!("server error: {message}"),
                Event::PeerLeft { .. } => {}
            }
        }
    }

    async fn expect_peer_left(&mut self, expected: &str) {
        loop {
            match self.next_event().await {
                Event::PeerLeft { session_id } => {
                    assert_eq!(session_id, expected);
                    return;
                }
                Event::ServerError { message } => panic!("server error: {message}"),
                Event::Media { .. } => {}
            }
        }
    }

    async fn next_event(&mut self) -> Event {
        timeout(STEP, self.events.recv())
            .await
            .expect("timed out waiting for an event")
            .expect("client stopped")
    }

    async fn hang_up(self) {
        let _ = self.hangup.send(());
        let _ = self.connection.close().await;
    }
}

/// Reports media from a stream once packets arrive on the section that was just
/// announced as carrying it.
fn report_when_flowing(
    (mid, stream_id): (String, String),
    arrivals: Arc<Mutex<HashMap<String, Instant>>>,
    events: mpsc::UnboundedSender<Event>,
) {
    let announced_at = Instant::now();
    tokio::spawn(async move {
        loop {
            let arrived = arrivals.lock().unwrap().get(&mid).is_some_and(|at| *at > announced_at);
            if arrived {
                let _ = events.send(Event::Media { stream_id });
                return;
            }
            if events.is_closed() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    });
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn participants_receive_each_others_media_including_late_joiners() {
    let server = TestServer::start().await;

    let mut alice = Client::join(&server, "standup", "alice").await;
    let mut bob = Client::join(&server, "standup", "bob").await;
    let (alice_id, bob_id) = (alice.session_id.clone(), bob.session_id.clone());
    assert_ne!(alice_id, bob_id, "the server assigns distinct session ids");

    alice.expect_media_from(&[&bob_id]).await;
    bob.expect_media_from(&[&alice_id]).await;

    // A third participant joins a room that already has two publishers and must
    // receive both of them; the others receive the newcomer.
    let mut carol = Client::join(&server, "standup", "carol").await;
    let carol_id = carol.session_id.clone();
    carol.expect_media_from(&[&alice_id, &bob_id]).await;
    alice.expect_media_from(&[&carol_id]).await;
    bob.expect_media_from(&[&carol_id]).await;

    // Someone in another room sees none of this.
    let mut outsider = Client::join(&server, "elsewhere", "erin").await;
    assert!(
        timeout(Duration::from_secs(2), outsider.events.recv()).await.is_err(),
        "a participant alone in another room receives nothing"
    );

    // When a participant disconnects, the others are told and the room keeps working.
    alice.hang_up().await;
    bob.expect_peer_left(&alice_id).await;
    carol.expect_peer_left(&alice_id).await;

    let mut dave = Client::join(&server, "standup", "dave").await;
    let dave_id = dave.session_id.clone();
    dave.expect_media_from(&[&bob_id, &carol_id]).await;
    bob.expect_media_from(&[&dave_id]).await;
    carol.expect_media_from(&[&dave_id]).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn media_sections_are_reused_when_participants_come_and_go() {
    let server = TestServer::start().await;
    let mut alice = Client::join(&server, "lobby", "alice").await;
    let mut bob = Client::join(&server, "lobby", "bob").await;
    let alice_id = alice.session_id.clone();
    bob.expect_media_from(&[&alice_id]).await;
    alice.expect_media_from(&[&bob.session_id.clone()]).await;

    let mut sections_with_a_guest = Vec::new();
    for round in 0..4 {
        let mut guest = Client::join(&server, "lobby", &format!("guest{round}")).await;
        let guest_id = guest.session_id.clone();

        // The guest's media reaches the others, on whichever section carries it.
        bob.expect_media_from(&[&guest_id]).await;
        alice.expect_media_from(&[&guest_id]).await;
        guest.expect_media_from(&[&alice_id, &bob.session_id.clone()]).await;
        sections_with_a_guest.push(bob.media_sections());

        guest.hang_up().await;
        bob.expect_peer_left(&guest_id).await;
        alice.expect_peer_left(&guest_id).await;
        // Let the server tell bob that the guest's section went quiet.
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    assert_eq!(
        sections_with_a_guest,
        vec![sections_with_a_guest[0]; 4],
        "four guests in a row must not leave four dead media sections behind"
    );
}

#[tokio::test]
async fn invalid_channel_ids_are_refused_and_bad_messages_get_an_error() {
    let server = TestServer::start().await;

    let cookie = server.login("alice", &["member"]).await;

    let refused =
        tokio_tungstenite::connect_async(server.ws_request(Some("not%20valid"), Some(&cookie)))
            .await;
    assert!(refused.is_err(), "upgrade must be refused");

    let without_channel = tokio_tungstenite::connect_async(server.ws_request(None, Some(&cookie)));
    assert!(without_channel.await.is_err(), "channel_id is required");

    let (mut socket, _) =
        tokio_tungstenite::connect_async(server.ws_request(Some("general"), Some(&cookie)))
            .await
            .unwrap();
    let hello = next_json(&mut socket).await;
    assert_eq!(hello["payload"]["channel_id"], "general");
    assert_eq!(hello["payload"]["participants"], json!([]));

    // A second participant sees the first in the roster, and the first is told.
    let bob = server.login("bob", &["member"]).await;
    let (mut second, _) =
        tokio_tungstenite::connect_async(server.ws_request(Some("general"), Some(&bob)))
            .await
            .unwrap();
    let second_hello = next_json(&mut second).await;
    assert_eq!(
        second_hello["payload"]["participants"],
        json!([{
            "session_id": hello["payload"]["session_id"],
            "username": "alice",
            "voice": { "muted": false, "camera": false, "screen": false, "speaking": false }
        }])
    );
    assert_eq!(
        next_json(&mut socket).await,
        json!({
            "channel": "peer_joined",
            "payload": {
                "session_id": second_hello["payload"]["session_id"],
                "username": "bob",
                "voice": { "muted": false, "camera": false, "screen": false, "speaking": false }
            }
        })
    );
    drop(second);
    assert_eq!(next_json(&mut socket).await["channel"], "peer_left");

    socket.send(Message::Text(r#"{"channel":"echo","payload":"ping"}"#.into())).await.unwrap();
    assert_eq!(next_json(&mut socket).await, json!({ "channel": "echo", "payload": "ping" }));

    // An answer nobody asked for, and something that is not a message at all.
    socket
        .send(Message::Text(
            r#"{"channel":"sdp_answer","payload":{"type":"answer","sdp":"v=0"}}"#.into(),
        ))
        .await
        .unwrap();
    assert_eq!(next_json(&mut socket).await["channel"], "error");

    socket.send(Message::Text("nonsense".into())).await.unwrap();
    assert_eq!(next_json(&mut socket).await["channel"], "error");
}

#[tokio::test]
async fn an_oversized_message_ends_that_connection_and_nothing_else() {
    let server = TestServer::start().await;
    server.voice_channel("big").await;
    let cookie = server.login("alice", &["member"]).await;
    let (mut socket, _) =
        tokio_tungstenite::connect_async(server.ws_request(Some("big"), Some(&cookie)))
            .await
            .unwrap();
    next_json(&mut socket).await;
    let bob = server.login("bob", &["member"]).await;
    let (mut other, _) =
        tokio_tungstenite::connect_async(server.ws_request(Some("big"), Some(&bob))).await.unwrap();
    next_json(&mut other).await;

    let frame = format!(r#"{{"channel":"echo","payload":"{}"}}"#, "a".repeat(200 * 1024));
    let _ = socket.send(Message::Text(frame.into())).await;
    // The server closes that socket: what follows is a close, an error or the end.
    let ended = timeout(STEP, async {
        while let Some(Ok(message)) = socket.next().await {
            assert!(!message.is_text() || !message.to_text().unwrap().contains("echo"));
        }
    })
    .await;
    assert!(ended.is_ok(), "the oversized message must end the connection");

    // The others carry on: bob is told that alice left, and can still talk.
    assert_eq!(next_json(&mut other).await["channel"], "peer_left");
    other.send(Message::Text(r#"{"channel":"echo","payload":"ping"}"#.into())).await.unwrap();
    assert_eq!(next_json(&mut other).await, json!({ "channel": "echo", "payload": "ping" }));
}
