//! End-to-end test: a receiver can play each participant's sound and picture in
//! step through the SFU. One publisher's audio and video share a stream id, and
//! nobody else's; and the sender reports the receiver gets carry the publisher's
//! clock, so a frame that left the publisher late (a busy encoder) still lines up
//! with the sound captured with it.

mod common;

use common::{STEP, TestServer, new_peer_connection_reporting_itself, next_json};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};
use tokio_tungstenite::tungstenite::Message;
use webrtc::api::media_engine::{MIME_TYPE_OPUS, MIME_TYPE_VP8};
use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::rtcp::packet::Packet as RtcpPacket;
use webrtc::rtcp::sender_report::SenderReport;
use webrtc::rtp::extension::abs_send_time_extension::unix2ntp;
use webrtc::rtp::header::Header;
use webrtc::rtp::packet::Packet;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::track::track_local::track_local_static_rtp::TrackLocalStaticRTP;
use webrtc::track::track_local::{TrackLocal, TrackLocalWriter};

/// How long after capture the publisher's video leaves it: an encoder under load.
const VIDEO_LAG_MS: u32 = 200;
/// How far apart a receiver may find sound and picture.
const TOLERANCE_MS: f64 = 30.0;

/// What a client last heard in a server offer.
#[derive(Default)]
struct Offered {
    sdp: String,
    /// `mid -> session id` of the publisher, from the `tracks` list.
    owners: HashMap<String, String>,
}

/// The latest sender report per media section, as the client received it.
type Reports = Arc<Mutex<HashMap<String, SenderReport>>>;

struct Client {
    session_id: String,
    _connection: Arc<RTCPeerConnection>,
    offered: Arc<Mutex<Offered>>,
    reports: Reports,
}

impl Client {
    /// Joins `channel`, publishing a microphone and a camera.
    async fn join(server: &TestServer, channel: &str, username: &str) -> Self {
        server.voice_channel(channel).await;
        let cookie = server.login(username, &["member"]).await;
        let request = server.ws_request(Some(channel), Some(&cookie));
        let (socket, _) = tokio_tungstenite::connect_async(request).await.expect("connect");
        let (mut ws_tx, mut ws_rx) = socket.split();
        let hello = next_json(&mut ws_rx).await;
        let session_id = hello["payload"]["session_id"].as_str().unwrap().to_owned();

        let connection = Arc::new(new_peer_connection_reporting_itself().await);
        publish(&connection).await;

        let reports = Reports::default();
        let on_track_reports = reports.clone();
        connection.on_track(Box::new(move |remote, receiver, transceiver| {
            let mid = transceiver.mid().map(|mid| mid.to_string()).unwrap_or_default();
            tokio::spawn(async move { while remote.read_rtp().await.is_ok() {} });
            let reports = on_track_reports.clone();
            tokio::spawn(async move {
                while let Ok((packets, _)) = receiver.read_rtcp().await {
                    for packet in &packets {
                        if let Some(report) = packet.as_any().downcast_ref::<SenderReport>() {
                            reports.lock().unwrap().insert(mid.clone(), report.clone());
                        }
                    }
                }
            });
            Box::pin(async {})
        }));

        let (out_tx, mut out_rx) = tokio::sync::mpsc::unbounded_channel::<Value>();
        let ice_out = out_tx.clone();
        connection.on_ice_candidate(Box::new(move |candidate| {
            if let Some(candidate) = candidate {
                let _ = ice_out
                    .send(json!({ "channel": "ice", "payload": candidate.to_json().unwrap() }));
            }
            Box::pin(async {})
        }));
        let offer = connection.create_offer(None).await.unwrap();
        out_tx.send(json!({ "channel": "sdp_offer", "payload": offer })).unwrap();
        connection.set_local_description(offer).await.unwrap();

        let offered = Arc::new(Mutex::new(Offered::default()));
        let (pc, last_offer) = (connection.clone(), offered.clone());
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
                        let payload = message["payload"].clone();
                        match message["channel"].as_str().unwrap() {
                            channel @ ("sdp_answer" | "sdp_offer") => {
                                if channel == "sdp_offer" {
                                    *last_offer.lock().unwrap() = offered_by(&payload);
                                }
                                let description: RTCSessionDescription =
                                    serde_json::from_value(payload).unwrap();
                                pc.set_remote_description(description).await.unwrap();
                                for candidate in early.drain(..) {
                                    pc.add_ice_candidate(candidate).await.unwrap();
                                }
                                if channel == "sdp_offer" {
                                    let answer = pc.create_answer(None).await.unwrap();
                                    pc.set_local_description(answer.clone()).await.unwrap();
                                    let _ = out_tx.send(json!({ "channel": "sdp_answer", "payload": answer }));
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
                }
            }
        });

        Self { session_id, _connection: connection, offered, reports }
    }

    /// The media sections the latest offer gives `publisher`.
    fn sections_of(&self, publisher: &str) -> Vec<String> {
        let offered = self.offered.lock().unwrap();
        let mut mids: Vec<String> = offered
            .owners
            .iter()
            .filter(|(_, owner)| owner.as_str() == publisher)
            .map(|(mid, _)| mid.clone())
            .collect();
        mids.sort();
        mids
    }

    /// The kind (`audio`, `video`) and `a=msid` stream id of each media section of
    /// the latest offer, by mid.
    fn stream_ids(&self) -> HashMap<String, (String, String)> {
        let offered = self.offered.lock().unwrap();
        offered
            .sdp
            .split("\r\nm=")
            .skip(1)
            .filter_map(|section| {
                let attribute = |name: &str| {
                    section.lines().find_map(|line| line.strip_prefix(name).map(str::to_owned))
                };
                let kind = section.split(' ').next()?.to_owned();
                let mid = attribute("a=mid:")?;
                let stream = attribute("a=msid:")?.split(' ').next()?.to_owned();
                Some((mid, (kind, stream)))
            })
            .collect()
    }
}

/// The SDP and section owners of a server offer.
fn offered_by(payload: &Value) -> Offered {
    let owners = payload["tracks"]
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
    Offered { sdp: payload["sdp"].as_str().unwrap().to_owned(), owners }
}

/// Sends a microphone (48 kHz) and a camera (90 kHz) whose timestamps count from
/// the same capture start. Video packets leave [`VIDEO_LAG_MS`] after capture, and
/// both streams' sender reports pair the capture clock with the wall clock, as a
/// browser's do.
async fn publish(connection: &Arc<RTCPeerConnection>) {
    let track = |mime: &str, id: &str| {
        Arc::new(TrackLocalStaticRTP::new(
            RTCRtpCodecCapability { mime_type: mime.to_owned(), ..Default::default() },
            id.to_owned(),
            "local".to_owned(),
        ))
    };
    let (audio, video) = (track(MIME_TYPE_OPUS, "microphone"), track(MIME_TYPE_VP8, "camera"));
    let mut ssrcs = Vec::new();
    for local in [&audio, &video] {
        let sender =
            connection.add_track(local.clone() as Arc<dyn TrackLocal + Send + Sync>).await.unwrap();
        ssrcs.push(sender.get_parameters().await.encodings[0].ssrc);
        tokio::spawn(async move { while sender.read_rtcp().await.is_ok() {} });
    }
    let (audio_ssrc, video_ssrc) = (ssrcs[0], ssrcs[1]);
    let connection = connection.clone();
    tokio::spawn(async move {
        let start = Instant::now();
        let mut sequence = 0u16;
        let mut last_report = Instant::now();
        loop {
            let now_ms = u32::try_from(start.elapsed().as_millis()).unwrap();
            let packet = |timestamp: u32, payload: &[u8]| Packet {
                header: Header {
                    version: 2,
                    sequence_number: sequence,
                    timestamp,
                    ..Default::default()
                },
                payload: payload.to_vec().into(),
            };
            if audio.write_rtp(&packet(now_ms * 48, &[0xf8, 0xff, 0xfe])).await.is_err() {
                break;
            }
            let captured = now_ms.saturating_sub(VIDEO_LAG_MS);
            let _ = video.write_rtp(&packet(captured * 90, &[0x10, 0x00, 0x00, 0x00])).await;
            sequence = sequence.wrapping_add(1);

            if last_report.elapsed() >= Duration::from_millis(500) {
                last_report = Instant::now();
                let ntp = unix2ntp(SystemTime::now());
                let report = |ssrc: u32, rtp_time: u32| -> Box<dyn RtcpPacket + Send + Sync> {
                    Box::new(SenderReport { ssrc, ntp_time: ntp, rtp_time, ..Default::default() })
                };
                let reports = [report(audio_ssrc, now_ms * 48), report(video_ssrc, now_ms * 90)];
                let _ = connection.write_rtcp(&reports).await;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    });
}

/// Where a sender report puts its stream at its own wall-clock time, in ms of
/// capture time minus ms of wall-clock time: the same for two streams in step.
#[allow(clippy::cast_precision_loss)]
fn lead_ms(report: &SenderReport, rate: f64) -> f64 {
    let wall_ms = report.ntp_time as f64 / 4_294_967.296;
    f64::from(report.rtp_time) / rate - wall_ms
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_receiver_gets_each_publishers_sound_and_picture_in_step() {
    let server = TestServer::start().await;
    let alice = Client::join(&server, "sync", "alice").await;
    let bob = Client::join(&server, "sync", "bob").await;
    let carol = Client::join(&server, "sync", "carol").await;

    let deadline = Instant::now() + STEP;
    let (alice_audio, alice_video) = loop {
        assert!(Instant::now() < deadline, "carol never got both sender reports of alice");
        let sections = carol.stream_ids();
        let mid_of = |publisher: &str, kind: &str| {
            carol
                .sections_of(publisher)
                .into_iter()
                .find(|mid| sections.get(mid).is_some_and(|(section_kind, _)| section_kind == kind))
        };
        if let (Some(audio), Some(video), Some(_), Some(_)) = (
            mid_of(&alice.session_id, "audio"),
            mid_of(&alice.session_id, "video"),
            mid_of(&bob.session_id, "audio"),
            mid_of(&bob.session_id, "video"),
        ) {
            let reports = carol.reports.lock().unwrap();
            if let (Some(audio), Some(video)) = (reports.get(&audio), reports.get(&video)) {
                break (audio.clone(), video.clone());
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    };

    let streams = carol.stream_ids();
    let stream_of = |publisher: &str| -> Vec<String> {
        carol.sections_of(publisher).iter().map(|mid| streams[mid].1.clone()).collect()
    };
    let (alices, bobs) = (stream_of(&alice.session_id), stream_of(&bob.session_id));
    assert_eq!(alices[0], alices[1], "alice's sound and picture share a stream id");
    assert_eq!(bobs[0], bobs[1], "bob's sound and picture share a stream id");
    assert_ne!(alices[0], bobs[0], "nobody is held back to match someone else");

    let skew = (lead_ms(&alice_audio, 48.0) - lead_ms(&alice_video, 90.0)).abs();
    assert!(
        skew < TOLERANCE_MS,
        "sound and picture are {skew:.0} ms apart; the publisher's video left {VIDEO_LAG_MS} ms after capture"
    );
}
