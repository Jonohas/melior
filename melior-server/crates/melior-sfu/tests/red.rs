//! End-to-end test: redundant audio (RED, RFC 2198) published by one client
//! reaches a client that speaks RED as RED, in that client's own numbering, and a
//! client that does not as plain Opus.

mod common;

use common::{STEP, TestServer, new_peer_connection_speaking, next_json};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use webrtc::api::media_engine::{MIME_TYPE_OPUS, MediaEngine};
use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::rtp::header::Header;
use webrtc::rtp::packet::Packet;
use webrtc::rtp_transceiver::RTCRtpTransceiverInit;
use webrtc::rtp_transceiver::rtp_codec::{
    RTCRtpCodecCapability, RTCRtpCodecParameters, RTPCodecType,
};
use webrtc::rtp_transceiver::rtp_transceiver_direction::RTCRtpTransceiverDirection;
use webrtc::track::track_local::track_local_static_rtp::TrackLocalStaticRTP;
use webrtc::track::track_local::{TrackLocal, TrackLocalWriter};

const MIME_TYPE_RED: &str = "audio/red";

/// A redundant block's first header byte: more blocks follow, then the payload type.
const fn follows(payload_type: u8) -> u8 {
    0x80 | payload_type
}

/// One redundant block (three bytes, Opus 111) and the primary block `[1, 2]`.
const RED_PAYLOAD: [u8; 10] = [follows(111), 0x00, 0x3c, 0x03, 111, 0xaa, 0xbb, 0xcc, 1, 2];

fn audio_codec(mime_type: &str, payload_type: u8, fmtp: &str) -> RTCRtpCodecParameters {
    RTCRtpCodecParameters {
        capability: RTCRtpCodecCapability {
            mime_type: mime_type.to_owned(),
            clock_rate: 48_000,
            channels: 2,
            sdp_fmtp_line: fmtp.to_owned(),
            rtcp_feedback: vec![],
        },
        payload_type,
        ..Default::default()
    }
}

/// Audio as a browser that prefers RED offers it, with Opus numbered `opus`.
fn red_first(opus: u8, red: u8) -> MediaEngine {
    let mut engine = MediaEngine::default();
    let fmtp = format!("{opus}/{opus}");
    engine.register_codec(audio_codec(MIME_TYPE_RED, red, &fmtp), RTPCodecType::Audio).unwrap();
    let opus_fmtp = "minptime=10;useinbandfec=1";
    engine
        .register_codec(audio_codec(MIME_TYPE_OPUS, opus, opus_fmtp), RTPCodecType::Audio)
        .unwrap();
    engine
}

fn opus_only() -> MediaEngine {
    let mut engine = MediaEngine::default();
    engine.register_default_codecs().unwrap();
    engine
}

/// What one RTP packet that reached a client looked like.
#[derive(Debug)]
struct Received {
    mime_type: String,
    payload: Vec<u8>,
}

/// Joins `channel` speaking `engine`'s codecs. A publisher sends RED packets on
/// its microphone; anyone else offers to receive audio only.
async fn join(
    server: &TestServer,
    channel: &str,
    username: &str,
    engine: MediaEngine,
    publish: bool,
) -> (Arc<RTCPeerConnection>, mpsc::UnboundedReceiver<Received>) {
    server.voice_channel(channel).await;
    let cookie = server.login(username, &["member"]).await;
    let request = server.ws_request(Some(channel), Some(&cookie));
    let (socket, _) = tokio_tungstenite::connect_async(request).await.expect("connect");
    let (mut ws_tx, mut ws_rx) = socket.split();
    assert_eq!(next_json(&mut ws_rx).await["channel"], "session");

    let connection = Arc::new(new_peer_connection_speaking(engine).await);
    if publish {
        publish_red(&connection).await;
    } else {
        let init = RTCRtpTransceiverInit {
            direction: RTCRtpTransceiverDirection::Recvonly,
            send_encodings: vec![],
        };
        connection.add_transceiver_from_kind(RTPCodecType::Audio, Some(init)).await.unwrap();
    }

    let (received_tx, received) = mpsc::unbounded_channel();
    connection.on_track(Box::new(move |remote, _, _| {
        let received = received_tx.clone();
        tokio::spawn(async move {
            while let Ok((packet, _)) = remote.read_rtp().await {
                let mime_type = remote.codec().capability.mime_type;
                let payload = packet.payload.to_vec();
                if received.send(Received { mime_type, payload }).is_err() {
                    return;
                }
            }
        });
        Box::pin(async {})
    }));

    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<Value>();
    let ice_out = out_tx.clone();
    connection.on_ice_candidate(Box::new(move |candidate| {
        if let Some(candidate) = candidate {
            let _ =
                ice_out.send(json!({ "channel": "ice", "payload": candidate.to_json().unwrap() }));
        }
        Box::pin(async {})
    }));
    let offer = connection.create_offer(None).await.unwrap();
    // Queued before gathering starts: no candidate may overtake it.
    out_tx.send(json!({ "channel": "sdp_offer", "payload": offer })).unwrap();
    connection.set_local_description(offer).await.unwrap();

    let pc = connection.clone();
    tokio::spawn(async move {
        let mut early: Vec<RTCIceCandidateInit> = Vec::new();
        loop {
            tokio::select! {
                outgoing = out_rx.recv() => {
                    let Some(outgoing) = outgoing else { break };
                    if ws_tx.send(Message::Text(outgoing.to_string().into())).await.is_err() {
                        break;
                    }
                }
                incoming = ws_rx.next() => {
                    let Some(Ok(Message::Text(text))) = incoming else { break };
                    let message: Value = serde_json::from_str(text.as_str()).unwrap();
                    handle(&pc, &message, &mut early, &out_tx).await;
                }
            }
        }
    });
    (connection, received)
}

/// One signalling message, as the browser client handles it.
async fn handle(
    pc: &RTCPeerConnection,
    message: &Value,
    early: &mut Vec<RTCIceCandidateInit>,
    out: &mpsc::UnboundedSender<Value>,
) {
    let payload = message["payload"].clone();
    match message["channel"].as_str().unwrap() {
        channel @ ("sdp_answer" | "sdp_offer") => {
            let description: RTCSessionDescription = serde_json::from_value(payload).unwrap();
            pc.set_remote_description(description).await.unwrap();
            for candidate in early.drain(..) {
                pc.add_ice_candidate(candidate).await.unwrap();
            }
            if channel == "sdp_offer" {
                let answer = pc.create_answer(None).await.unwrap();
                pc.set_local_description(answer.clone()).await.unwrap();
                let _ = out.send(json!({ "channel": "sdp_answer", "payload": answer }));
            }
        }
        "ice" => {
            let candidate: RTCIceCandidateInit = serde_json::from_value(payload).unwrap();
            if pc.remote_description().await.is_some() {
                pc.add_ice_candidate(candidate).await.unwrap();
            } else {
                early.push(candidate);
            }
        }
        "error" => panic!("server error: {payload}"),
        _ => {}
    }
}

/// Adds a RED microphone and keeps sending [`RED_PAYLOAD`] on it.
async fn publish_red(connection: &RTCPeerConnection) {
    let track = Arc::new(TrackLocalStaticRTP::new(
        audio_codec(MIME_TYPE_RED, 63, "111/111").capability,
        "microphone".to_owned(),
        "local".to_owned(),
    ));
    connection.add_track(track.clone() as Arc<dyn TrackLocal + Send + Sync>).await.unwrap();
    tokio::spawn(async move {
        for sequence_number in 0u16.. {
            let packet = Packet {
                header: Header {
                    version: 2,
                    sequence_number,
                    timestamp: u32::from(sequence_number) * 960,
                    ..Default::default()
                },
                payload: RED_PAYLOAD.to_vec().into(),
            };
            if track.write_rtp(&packet).await.is_err() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    });
}

async fn first_packet(received: &mut mpsc::UnboundedReceiver<Received>) -> Received {
    timeout(STEP, received.recv()).await.expect("no audio in time").expect("client stopped")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn red_reaches_each_listener_in_the_form_it_speaks() {
    let server = TestServer::start().await;
    let (_speaker, _) = join(&server, "red", "alice", red_first(111, 63), true).await;
    // Numbered as Firefox numbers Opus, unlike the speaker.
    let (_red_listener, mut red) = join(&server, "red", "bob", red_first(109, 120), false).await;
    let (_opus_listener, mut plain) = join(&server, "red", "carol", opus_only(), false).await;

    let packet = first_packet(&mut red).await;
    assert_eq!(packet.mime_type, MIME_TYPE_RED);
    let mut expected = RED_PAYLOAD;
    expected[0] = follows(109);
    expected[4] = 109;
    assert_eq!(packet.payload, expected, "the blocks name the listener's own Opus");

    let packet = first_packet(&mut plain).await;
    assert_eq!(packet.mime_type, MIME_TYPE_OPUS);
    assert_eq!(packet.payload, [1, 2], "only the current frame, as plain Opus");
}
