//! End-to-end test: a participant that stops sharing its screen by renegotiating
//! (the section stays in the SDP but no longer sends) must have the screen
//! retracted for everyone, and must be able to share again.

mod common;

use common::{STEP, TestServer, new_peer_connection, next_json};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use webrtc::api::media_engine::MIME_TYPE_VP8;
use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::rtp::header::Header;
use webrtc::rtp::packet::Packet;
use webrtc::rtp_transceiver::RTCRtpTransceiverInit;
use webrtc::rtp_transceiver::rtp_codec::{RTCRtpCodecCapability, RTPCodecType};
use webrtc::rtp_transceiver::rtp_sender::RTCRtpSender;
use webrtc::track::track_local::track_local_static_rtp::TrackLocalStaticRTP;
use webrtc::track::track_local::{TrackLocal, TrackLocalWriter};

/// What a client heard from the server.
enum Note {
    /// The server's answer to the client's own offer.
    Answer,
    /// A server offer: the `tracks` list next to the SDP.
    Offer(Vec<Value>),
    Error(String),
}

struct Peer {
    connection: Arc<RTCPeerConnection>,
    outgoing: mpsc::UnboundedSender<Value>,
    notes: mpsc::UnboundedReceiver<Note>,
    /// The last offer sent, still describing every track the client sends.
    last_offer: Mutex<String>,
}

impl Peer {
    /// Joins `channel`; sends the first offer, with a camera track when `camera`.
    async fn join(server: &TestServer, channel: &str, username: &str, camera: bool) -> Self {
        let cookie = server.login(username, &["member"]).await;
        let request = server.ws_request(Some(channel), Some(&cookie));
        let (socket, _) = tokio_tungstenite::connect_async(request).await.expect("connect");
        let (ws_tx, mut ws_rx) = socket.split();
        assert_eq!(next_json(&mut ws_rx).await["channel"], "session");

        let connection = Arc::new(new_peer_connection().await);
        let (outgoing, out_rx) = mpsc::unbounded_channel::<Value>();
        let (notes_tx, notes) = mpsc::unbounded_channel();

        let (candidates, candidates_rx) = mpsc::unbounded_channel::<Value>();
        connection.on_ice_candidate(Box::new(move |candidate| {
            if let Some(candidate) = candidate {
                let init = candidate.to_json().unwrap();
                let _ = candidates.send(json!({ "channel": "ice", "payload": init }));
            }
            Box::pin(async {})
        }));

        let peer = Self {
            connection: connection.clone(),
            outgoing: outgoing.clone(),
            notes,
            last_offer: Mutex::default(),
        };
        if camera {
            let (track, _) = peer.add_video("camera", "camera").await;
            spawn_rtp_source(track);
        } else {
            connection
                .add_transceiver_from_kind(
                    RTPCodecType::Video,
                    Some(RTCRtpTransceiverInit {
                        direction: webrtc::rtp_transceiver::rtp_transceiver_direction::RTCRtpTransceiverDirection::Recvonly,
                        send_encodings: vec![],
                    }),
                )
                .await
                .unwrap();
        }
        // The offer is queued before gathering starts: no candidate may overtake it.
        let offer = connection.create_offer(None).await.unwrap();
        outgoing.send(json!({ "channel": "sdp_offer", "payload": offer })).unwrap();
        peer.last_offer.lock().unwrap().clone_from(&offer.sdp);
        connection.set_local_description(offer).await.unwrap();
        release_candidates(candidates_rx, outgoing.clone());

        tokio::spawn(signalling(connection, outgoing, ws_tx, ws_rx, out_rx, notes_tx));
        peer
    }

    /// Adds a video track on a media section of its own.
    async fn add_video(
        &self,
        id: &str,
        stream: &str,
    ) -> (Arc<TrackLocalStaticRTP>, Arc<RTCRtpSender>) {
        let track = Arc::new(TrackLocalStaticRTP::new(
            RTCRtpCodecCapability { mime_type: MIME_TYPE_VP8.to_owned(), ..Default::default() },
            id.to_owned(),
            stream.to_owned(),
        ));
        let transceiver = self
            .connection
            .add_transceiver_from_track(track.clone() as Arc<dyn TrackLocal + Send + Sync>, None)
            .await
            .unwrap();
        (track, transceiver.sender().await)
    }

    /// Sends a fresh offer, like the browser after adding or removing a track.
    async fn renegotiate(&self) {
        let offer = self.connection.create_offer(None).await.unwrap();
        self.outgoing.send(json!({ "channel": "sdp_offer", "payload": offer })).unwrap();
        self.last_offer.lock().unwrap().clone_from(&offer.sdp);
        self.connection.set_local_description(offer).await.unwrap();
    }

    /// Sends the offer that follows a removed screen track, as `how` shows it to the
    /// server. The section keeps its `a=ssrc` and `a=msid` lines, as a browser's
    /// does: only its direction (and port) change.
    async fn renegotiate_stopped(&self, how: Stopped) {
        let offer = self.connection.create_offer(None).await.unwrap();
        let mut sent = offer.clone();
        sent.sdp = how.apply(&self.last_offer.lock().unwrap());
        self.outgoing.send(json!({ "channel": "sdp_offer", "payload": sent })).unwrap();
        self.connection.set_local_description(offer).await.unwrap();
    }

    /// Reads notes in order until one satisfies `wanted`; earlier ones are skipped.
    /// A server `error` fails the test.
    async fn wait_for<T>(&mut self, what: &str, wanted: impl Fn(&Note) -> Option<T>) -> T {
        loop {
            let note = timeout(STEP, self.notes.recv())
                .await
                .unwrap_or_else(|_| panic!("timed out waiting for {what}"))
                .expect("client stopped");
            if let Some(found) = wanted(&note) {
                return found;
            }
            if let Note::Error(message) = note {
                panic!("server error while waiting for {what}: {message}");
            }
        }
    }

    async fn wait_for_answer(&mut self) {
        self.wait_for("the answer", |note| matches!(note, Note::Answer).then_some(())).await;
    }

    /// Waits for a server offer whose sources are exactly `expected` (sorted).
    async fn wait_for_offer_with(&mut self, expected: &[&str]) {
        let what = format!("an offer with sources {expected:?}");
        self.wait_for(&what, |note| {
            let Note::Offer(tracks) = note else { return None };
            let mut sources: Vec<&str> =
                tracks.iter().map(|track| track["source"].as_str().unwrap_or("?")).collect();
            sources.sort_unstable();
            (sources == expected).then_some(())
        })
        .await;
    }
}

/// How a client's offer shows that a section has stopped sending.
#[derive(Clone, Copy, Debug)]
enum Stopped {
    /// Left as the browser offers it after `removeTrack`: `a=recvonly`.
    ReceiveOnly,
    /// `a=inactive`.
    Inactive,
    /// The section rejected: port 0 and `a=inactive`.
    Rejected,
}

impl Stopped {
    /// Rewrites the second section (the screen's) of an offer in which it sends.
    fn apply(self, sdp: &str) -> String {
        let (direction, port) = match self {
            Self::ReceiveOnly => ("a=recvonly", "9"),
            Self::Inactive => ("a=inactive", "9"),
            Self::Rejected => ("a=inactive", "0"),
        };
        let mut section = 0;
        let lines: Vec<String> = sdp
            .split("\r\n")
            .filter(|line| !line.is_empty())
            .map(|line| {
                if line.starts_with("m=") {
                    section += 1;
                }
                match (section == 2, line) {
                    (true, "a=sendrecv") => direction.to_owned(),
                    (true, _) if line.starts_with("m=video 9 ") => {
                        line.replacen("m=video 9 ", &format!("m=video {port} "), 1)
                    }
                    _ => line.to_owned(),
                }
            })
            .collect();
        lines.join("\r\n") + "\r\n"
    }
}

/// Keeps sending VP8 RTP on `track`, so the server sees the track and forwards it.
fn spawn_rtp_source(track: Arc<TrackLocalStaticRTP>) {
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
}

/// Sends the queued local candidates, after everything already in `outgoing`.
fn release_candidates(
    mut candidates: mpsc::UnboundedReceiver<Value>,
    outgoing: mpsc::UnboundedSender<Value>,
) {
    tokio::spawn(async move {
        while let Some(message) = candidates.recv().await {
            if outgoing.send(message).is_err() {
                break;
            }
        }
    });
}

/// The signalling loop: one message at a time, like the browser client.
async fn signalling<S, R>(
    connection: Arc<RTCPeerConnection>,
    outgoing: mpsc::UnboundedSender<Value>,
    mut ws_tx: S,
    mut ws_rx: R,
    mut out_rx: mpsc::UnboundedReceiver<Value>,
    notes: mpsc::UnboundedSender<Note>,
) where
    S: SinkExt<Message> + Unpin,
    R: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    let mut early: Vec<RTCIceCandidateInit> = Vec::new();
    loop {
        tokio::select! {
            message = out_rx.recv() => {
                let Some(message) = message else { break };
                if ws_tx.send(Message::Text(message.to_string().into())).await.is_err() {
                    break;
                }
            }
            incoming = ws_rx.next() => {
                let Some(Ok(Message::Text(text))) = incoming else { break };
                let message: Value = serde_json::from_str(text.as_str()).unwrap();
                let payload = message["payload"].clone();
                match message["channel"].as_str().unwrap() {
                    "sdp_answer" => {
                        let description: RTCSessionDescription =
                            serde_json::from_value(payload).unwrap();
                        connection.set_remote_description(description).await.unwrap();
                        flush(&connection, &mut early).await;
                        let _ = notes.send(Note::Answer);
                    }
                    "sdp_offer" => {
                        let tracks = payload["tracks"].as_array().cloned().unwrap_or_default();
                        let description: RTCSessionDescription =
                            serde_json::from_value(payload).unwrap();
                        connection.set_remote_description(description).await.unwrap();
                        flush(&connection, &mut early).await;
                        let answer = connection.create_answer(None).await.unwrap();
                        connection.set_local_description(answer.clone()).await.unwrap();
                        let _ = outgoing.send(json!({ "channel": "sdp_answer", "payload": answer }));
                        let _ = notes.send(Note::Offer(tracks));
                    }
                    "ice" => {
                        let candidate: RTCIceCandidateInit =
                            serde_json::from_value(payload).unwrap();
                        if connection.remote_description().await.is_some() {
                            connection.add_ice_candidate(candidate).await.unwrap();
                        } else {
                            early.push(candidate);
                        }
                    }
                    "error" => {
                        let message = payload["message"].as_str().unwrap().to_owned();
                        let _ = notes.send(Note::Error(message));
                    }
                    _ => {}
                }
            }
        }
    }
    let _ = connection.close().await;
}

async fn flush(connection: &RTCPeerConnection, early: &mut Vec<RTCIceCandidateInit>) {
    for candidate in early.drain(..) {
        connection.add_ice_candidate(candidate).await.unwrap();
    }
}

async fn screen_is_retracted_and_shared_again(stopped: Stopped) {
    let server = TestServer::start().await;
    let mut alice = Peer::join(&server, "share", "alice", true).await;
    alice.wait_for_answer().await;
    let mut bob = Peer::join(&server, "share", "bob", false).await;
    bob.wait_for_answer().await;
    bob.wait_for_offer_with(&["camera"]).await;

    // Alice shares her screen: a second video track on a section of its own.
    let (screen, sender) = alice.add_video("screen", "screen").await;
    spawn_rtp_source(screen);
    alice.renegotiate().await;
    alice.wait_for_answer().await;
    bob.wait_for_offer_with(&["camera", "screen"]).await;

    // She stops: the section stays in her offer but no longer sends.
    alice.connection.remove_track(&sender).await.unwrap();
    alice.renegotiate_stopped(stopped).await;
    alice.wait_for_answer().await;
    bob.wait_for_offer_with(&["camera"]).await;

    // And shares again: the stale screen no longer counts as a second video.
    let (screen, _) = alice.add_video("screen", "screen").await;
    spawn_rtp_source(screen);
    alice.renegotiate().await;
    alice.wait_for_answer().await;
    bob.wait_for_offer_with(&["camera", "screen"]).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_receive_only_screen_is_retracted_and_can_be_shared_again() {
    screen_is_retracted_and_shared_again(Stopped::ReceiveOnly).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_inactive_screen_is_retracted_and_can_be_shared_again() {
    screen_is_retracted_and_shared_again(Stopped::Inactive).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_rejected_screen_is_retracted_and_can_be_shared_again() {
    screen_is_retracted_and_shared_again(Stopped::Rejected).await;
}
