//! A test client that behaves like the browser client with the mesh: an SFU
//! connection that exists for the whole session, plus one direct connection per peer
//! with an open link. It reports what it observes as [`Event`]s and leaves every
//! decision (when to report a route as live, what to send) to the test.
#![allow(dead_code)]

use crate::common::{STEP, TestServer, new_peer_connection, next_json};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use webrtc::api::media_engine::MIME_TYPE_VP8;
use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::rtp::header::Header;
use webrtc::rtp::packet::Packet;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::track::track_local::track_local_static_rtp::TrackLocalStaticRTP;
use webrtc::track::track_local::{TrackLocal, TrackLocalWriter};

/// Something a test client observed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Route {
        peer: String,
        path: String,
        epoch: u64,
    },
    LinkClose {
        peer: String,
    },
    Uplink {
        active: bool,
    },
    /// RTP arrived on the direct connection to `peer`.
    DirectMedia {
        peer: String,
    },
    /// RTP arrived on the SFU connection, on a section announced as carrying `owner`.
    SfuMedia {
        owner: String,
    },
    /// A relayed `peer_*` message arrived: its channel name and its SDP or candidate text.
    PeerSignal {
        channel: String,
        body: String,
    },
    PeerLeft {
        session_id: String,
    },
    Echo(String),
    ServerError {
        message: String,
    },
}

pub struct MeshClient {
    pub session_id: String,
    events: mpsc::UnboundedReceiver<Event>,
    backlog: VecDeque<Event>,
    outgoing: mpsc::UnboundedSender<Value>,
    /// Dropping this closes the WebSocket, like closing the browser tab.
    hangup: mpsc::UnboundedSender<()>,
}

impl MeshClient {
    /// Logs `username` in as a member and joins `channel`.
    pub async fn join(server: &TestServer, channel: &str, username: &str) -> Self {
        server.voice_channel(channel).await;
        let cookie = server.login(username, &["member"]).await;
        let request = server.ws_request(Some(channel), Some(&cookie));
        let (socket, _) = tokio_tungstenite::connect_async(request).await.expect("connect");
        let (ws_tx, mut ws_rx) = socket.split();

        let hello = next_json(&mut ws_rx).await;
        assert_eq!(hello["channel"], "session");
        let session_id = hello["payload"]["session_id"].as_str().unwrap().to_owned();

        let (events_tx, events) = mpsc::unbounded_channel();
        let (outgoing, out_rx) = mpsc::unbounded_channel::<Value>();
        let (hangup, hangup_rx) = mpsc::unbounded_channel::<()>();

        let anchor = Arc::new(new_peer_connection().await);
        publish_camera(&anchor).await;
        let candidates = queue_candidates(&anchor, "ice", None);
        let arrivals = watch_sfu_arrivals(&anchor);
        let offer = anchor.create_offer(None).await.unwrap();
        anchor.set_local_description(offer.clone()).await.unwrap();
        outgoing.send(json!({ "channel": "sdp_offer", "payload": offer })).unwrap();
        release_candidates(candidates, &outgoing);

        let signalling = Signalling {
            my_id: session_id.clone(),
            anchor,
            outgoing: outgoing.clone(),
            events: events_tx,
            arrivals,
            announced: HashSet::new(),
            early_anchor: Vec::new(),
            direct: HashMap::new(),
        };
        tokio::spawn(signalling.run(ws_tx, ws_rx, out_rx, hangup_rx));
        Self { session_id, events, backlog: VecDeque::new(), outgoing, hangup }
    }

    pub fn send(&self, message: &Value) {
        self.outgoing.send(message.clone()).unwrap();
    }

    pub fn route_live(&self, peer: &str, epoch: u64) {
        self.send(&json!({ "channel": "route_live", "payload": { "peer": peer, "epoch": epoch } }));
    }

    pub fn link_failed(&self, peer: &str) {
        self.send(&json!({ "channel": "link_failed", "payload": { "peer": peer } }));
    }

    /// Waits for the first event (already seen or still to come) that satisfies
    /// `wanted`. Other events stay queued for later calls. A server `error` fails
    /// the test unless it is what is wanted.
    pub async fn expect(&mut self, what: &str, wanted: impl Fn(&Event) -> bool) -> Event {
        if let Some(index) = self.backlog.iter().position(&wanted) {
            return self.backlog.remove(index).unwrap();
        }
        loop {
            let event = timeout(STEP, self.events.recv())
                .await
                .unwrap_or_else(|_| {
                    panic!("timed out waiting for {what}; seen: {:?}", self.backlog)
                })
                .expect("client stopped");
            if wanted(&event) {
                return event;
            }
            if let Event::ServerError { message } = &event {
                panic!("server error while waiting for {what}: {message}");
            }
            self.backlog.push_back(event);
        }
    }

    /// Waits for a route to `peer` over `path`; returns its epoch.
    pub async fn expect_route(&mut self, peer: &str, path: &str) -> u64 {
        let event = self
            .expect(&format!("route {path} for {peer}"), |event| {
                matches!(event, Event::Route { peer: p, path: q, .. } if p == peer && q == path)
            })
            .await;
        match event {
            Event::Route { epoch, .. } => epoch,
            other => panic!("expected a route, got {other:?}"),
        }
    }

    pub async fn expect_direct_media(&mut self, peer: &str) {
        self.expect(
            &format!("direct media from {peer}"),
            |event| matches!(event, Event::DirectMedia { peer: p } if p == peer),
        )
        .await;
    }

    pub async fn expect_sfu_media(&mut self, owner: &str) {
        self.expect(
            &format!("SFU media from {owner}"),
            |event| matches!(event, Event::SfuMedia { owner: o } if o == owner),
        )
        .await;
    }

    pub async fn expect_uplink(&mut self, active: bool) {
        self.expect(
            &format!("uplink active={active}"),
            |event| matches!(event, Event::Uplink { active: a } if *a == active),
        )
        .await;
    }

    pub async fn expect_link_close(&mut self, peer: &str) {
        self.expect(
            &format!("link_close for {peer}"),
            |event| matches!(event, Event::LinkClose { peer: p } if p == peer),
        )
        .await;
    }

    /// Whether a relayed message whose text contains `marker` has arrived.
    pub fn saw_relay_containing(&mut self, marker: &str) -> bool {
        self.unclaimed()
            .iter()
            .any(|event| matches!(event, Event::PeerSignal { body, .. } if body.contains(marker)))
    }

    /// Round-trips an `echo`: the server handles a socket's messages in order, so
    /// once the reply is here everything sent earlier has been processed and its
    /// effects have been queued for the other clients.
    pub async fn sync(&mut self) {
        let token = format!("sync-{}", self.session_id);
        self.send(&json!({ "channel": "echo", "payload": token }));
        self.expect("an echo", |event| *event == Event::Echo(token.clone())).await;
    }

    /// Everything observed so far that no `expect` has taken.
    pub fn unclaimed(&mut self) -> Vec<Event> {
        while let Ok(event) = self.events.try_recv() {
            self.backlog.push_back(event);
        }
        self.backlog.iter().cloned().collect()
    }

    pub fn hang_up(self) {
        let _ = self.hangup.send(());
    }
}

/// Joins two clients to `channel` on a mesh server and brings their direct link to
/// `Direct`: both play the peer's media over both paths and report `route_live`,
/// and both are told to pause their SFU uplink.
pub async fn direct_pair(server: &TestServer, channel: &str) -> (MeshClient, MeshClient) {
    let mut first = MeshClient::join(server, channel, "alice").await;
    let mut second = MeshClient::join(server, channel, "bob").await;
    let (a, b) = (first.session_id.clone(), second.session_id.clone());

    let epoch = first.expect_route(&b, "direct").await;
    assert_eq!(second.expect_route(&a, "direct").await, epoch);
    for (client, peer) in [(&mut first, &b), (&mut second, &a)] {
        client.expect_direct_media(peer).await;
        client.expect_sfu_media(peer).await;
        client.route_live(peer, epoch);
    }
    first.expect_uplink(false).await;
    second.expect_uplink(false).await;
    (first, second)
}

type Arrivals = Arc<Mutex<HashMap<String, Instant>>>;

/// The per-socket signalling state, driven one message at a time like the browser.
struct Signalling {
    my_id: String,
    anchor: Arc<RTCPeerConnection>,
    outgoing: mpsc::UnboundedSender<Value>,
    events: mpsc::UnboundedSender<Event>,
    arrivals: Arrivals,
    announced: HashSet<(String, String)>,
    early_anchor: Vec<RTCIceCandidateInit>,
    direct: HashMap<String, DirectLink>,
}

struct DirectLink {
    connection: Arc<RTCPeerConnection>,
    early: Vec<RTCIceCandidateInit>,
    /// Local candidates, held back until the description they belong to is sent.
    candidates: Option<mpsc::UnboundedReceiver<Value>>,
}

impl Signalling {
    async fn run(
        mut self,
        mut ws_tx: impl SinkExt<Message> + Unpin,
        mut ws_rx: impl StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
        mut out_rx: mpsc::UnboundedReceiver<Value>,
        mut hangup_rx: mpsc::UnboundedReceiver<()>,
    ) {
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
                    let channel = text_of(&message["channel"]);
                    self.handle(&channel, message["payload"].clone()).await;
                }
            }
        }
        for link in self.direct.values() {
            let _ = link.connection.close().await;
        }
        let _ = self.anchor.close().await;
    }

    fn emit(&self, event: Event) {
        let _ = self.events.send(event);
    }

    async fn handle(&mut self, channel: &str, payload: Value) {
        match channel {
            "sdp_answer" | "sdp_offer" => self.on_anchor_description(channel, payload).await,
            "ice" => self.on_anchor_candidate(payload).await,
            "route" => self.on_route(&payload).await,
            "link_close" => self.on_link_close(&payload).await,
            "uplink" => self.emit(Event::Uplink { active: payload["active"].as_bool().unwrap() }),
            "peer_offer" | "peer_answer" | "peer_ice" => {
                self.on_peer_signal(channel, &payload).await;
            }
            "peer_left" => {
                self.emit(Event::PeerLeft { session_id: text_of(&payload["session_id"]) });
            }
            "peer_joined" => {}
            "echo" => self.emit(Event::Echo(text_of(&payload))),
            "error" => self.emit(Event::ServerError { message: text_of(&payload["message"]) }),
            other => panic!("unexpected channel {other}"),
        }
    }

    async fn on_anchor_description(&mut self, channel: &str, payload: Value) {
        let is_offer = channel == "sdp_offer";
        let sections = is_offer.then(|| announced_sections(&payload));
        let description: RTCSessionDescription = serde_json::from_value(payload).unwrap();
        self.anchor.set_remote_description(description).await.unwrap();
        if let Some(sections) = sections {
            self.announced.retain(|section| sections.contains(section));
            for section in sections {
                if self.announced.insert(section.clone()) {
                    report_when_flowing(section, self.arrivals.clone(), self.events.clone());
                }
            }
        }
        for candidate in self.early_anchor.drain(..) {
            self.anchor.add_ice_candidate(candidate).await.unwrap();
        }
        if is_offer {
            let answer = self.anchor.create_answer(None).await.unwrap();
            self.anchor.set_local_description(answer.clone()).await.unwrap();
            let _ = self.outgoing.send(json!({ "channel": "sdp_answer", "payload": answer }));
        }
    }

    async fn on_anchor_candidate(&mut self, payload: Value) {
        let candidate: RTCIceCandidateInit = serde_json::from_value(payload).unwrap();
        if self.anchor.remote_description().await.is_some() {
            self.anchor.add_ice_candidate(candidate).await.unwrap();
        } else {
            self.early_anchor.push(candidate);
        }
    }

    async fn on_route(&mut self, payload: &Value) {
        let peer = text_of(&payload["peer"]);
        let path = text_of(&payload["path"]);
        let epoch = payload["epoch"].as_u64().unwrap();
        if path == "direct" && !self.direct.contains_key(&peer) {
            self.open_link(&peer).await;
            if self.my_id < peer {
                self.offer(&peer).await;
            }
        }
        self.emit(Event::Route { peer, path, epoch });
    }

    async fn on_link_close(&mut self, payload: &Value) {
        let peer = text_of(&payload["peer"]);
        if let Some(link) = self.direct.remove(&peer) {
            let _ = link.connection.close().await;
        }
        self.emit(Event::LinkClose { peer });
    }

    async fn open_link(&mut self, peer: &str) {
        let connection = Arc::new(new_peer_connection().await);
        let candidates = queue_candidates(&connection, "peer_ice", Some(peer.to_owned()));
        let (events, remote) = (self.events.clone(), peer.to_owned());
        connection.on_track(Box::new(move |track, _, _| {
            let (events, peer) = (events.clone(), remote.clone());
            tokio::spawn(async move {
                if track.read_rtp().await.is_ok() {
                    let _ = events.send(Event::DirectMedia { peer });
                }
                while track.read_rtp().await.is_ok() {}
            });
            Box::pin(async {})
        }));
        self.direct.insert(
            peer.to_owned(),
            DirectLink { connection, early: vec![], candidates: Some(candidates) },
        );
    }

    /// The lower session id's side makes the offer.
    async fn offer(&mut self, peer: &str) {
        let connection = self.direct[peer].connection.clone();
        publish_camera(&connection).await;
        let offer = connection.create_offer(None).await.unwrap();
        connection.set_local_description(offer.clone()).await.unwrap();
        let _ = self
            .outgoing
            .send(json!({ "channel": "peer_offer", "payload": { "to": peer, "sdp": offer.sdp } }));
    }

    /// A candidate must not overtake the description it belongs to: the other side
    /// drops candidates for a link it has not heard of.
    fn release_candidates(&mut self, peer: &str) {
        let Some(candidates) = self.direct.get_mut(peer).and_then(|link| link.candidates.take())
        else {
            return;
        };
        release_candidates(candidates, &self.outgoing);
    }

    /// Handles a relayed message tolerantly: a late one for a closed link is normal.
    async fn on_peer_signal(&mut self, channel: &str, payload: &Value) {
        let body = payload["sdp"].as_str().or(payload["candidate"]["candidate"].as_str());
        self.emit(Event::PeerSignal {
            channel: channel.to_owned(),
            body: body.unwrap_or_default().to_owned(),
        });
        let from = text_of(&payload["from"]);
        match channel {
            "peer_offer" => self.answer(&from, text_of(&payload["sdp"])).await,
            "peer_answer" => {
                let Some(link) = self.direct.get_mut(&from) else { return };
                let answer = RTCSessionDescription::answer(text_of(&payload["sdp"])).unwrap();
                link.connection.set_remote_description(answer).await.unwrap();
                flush_early(link).await;
            }
            _ => {
                let Some(link) = self.direct.get_mut(&from) else { return };
                let candidate: RTCIceCandidateInit =
                    serde_json::from_value(payload["candidate"].clone()).unwrap();
                if link.connection.remote_description().await.is_some() {
                    let _ = link.connection.add_ice_candidate(candidate).await;
                } else {
                    link.early.push(candidate);
                }
            }
        }
    }

    /// The answerer sets the remote offer first and adds its track after, so the
    /// track reuses the offer's transceiver.
    async fn answer(&mut self, peer: &str, sdp: String) {
        if !self.direct.contains_key(peer) {
            self.open_link(peer).await;
        }
        let connection = self.direct[peer].connection.clone();
        let offer = RTCSessionDescription::offer(sdp).unwrap();
        publish_camera(&connection).await;
        connection.set_remote_description(offer).await.unwrap();
        let answer = connection.create_answer(None).await.unwrap();
        connection.set_local_description(answer.clone()).await.unwrap();
        let _ = self.outgoing.send(
            json!({ "channel": "peer_answer", "payload": { "to": peer, "sdp": answer.sdp } }),
        );
        self.release_candidates(peer);
        if let Some(link) = self.direct.get_mut(peer) {
            flush_early(link).await;
        }
    }
}

async fn flush_early(link: &mut DirectLink) {
    for candidate in link.early.drain(..) {
        let _ = link.connection.add_ice_candidate(candidate).await;
    }
}

fn text_of(value: &Value) -> String {
    value.as_str().unwrap().to_owned()
}

/// What a server offer says: which media sections carry whose track.
fn announced_sections(payload: &Value) -> HashSet<(String, String)> {
    payload["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|owner| (text_of(&owner["mid"]), text_of(&owner["session_id"])))
        .collect()
}

/// Collects every local ICE candidate as a `channel` message, addressed to `to`
/// when given. Nothing is sent until [`release_candidates`].
fn queue_candidates(
    connection: &RTCPeerConnection,
    channel: &'static str,
    to: Option<String>,
) -> mpsc::UnboundedReceiver<Value> {
    let (queue, candidates) = mpsc::unbounded_channel();
    connection.on_ice_candidate(Box::new(move |candidate| {
        if let Some(candidate) = candidate {
            let init = candidate.to_json().unwrap();
            let payload = match &to {
                Some(to) => json!({ "to": to, "candidate": init }),
                None => json!(init),
            };
            let _ = queue.send(json!({ "channel": channel, "payload": payload }));
        }
        Box::pin(async {})
    }));
    candidates
}

/// Starts sending the queued candidates, after everything already in `outgoing`.
fn release_candidates(
    mut candidates: mpsc::UnboundedReceiver<Value>,
    outgoing: &mpsc::UnboundedSender<Value>,
) {
    let outgoing = outgoing.clone();
    tokio::spawn(async move {
        while let Some(message) = candidates.recv().await {
            if outgoing.send(message).is_err() {
                break;
            }
        }
    });
}

/// Records when RTP last arrived on each media section of the SFU connection.
fn watch_sfu_arrivals(anchor: &RTCPeerConnection) -> Arrivals {
    let arrivals = Arrivals::default();
    let recorded = arrivals.clone();
    anchor.on_track(Box::new(move |remote, _, transceiver| {
        let arrivals = recorded.clone();
        let mid = transceiver.mid().map(|mid| mid.to_string()).unwrap_or_default();
        tokio::spawn(async move {
            while remote.read_rtp().await.is_ok() {
                arrivals.lock().unwrap().insert(mid.clone(), Instant::now());
            }
        });
        Box::pin(async {})
    }));
    arrivals
}

/// Reports media from an owner once packets arrive on the section that was just
/// announced as carrying it.
fn report_when_flowing(
    (mid, owner): (String, String),
    arrivals: Arrivals,
    events: mpsc::UnboundedSender<Event>,
) {
    let announced_at = Instant::now();
    tokio::spawn(async move {
        loop {
            let arrived = arrivals.lock().unwrap().get(&mid).is_some_and(|at| *at > announced_at);
            if arrived {
                let _ = events.send(Event::SfuMedia { owner });
                return;
            }
            if events.is_closed() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    });
}

/// Adds a VP8 track to `connection` and keeps sending RTP on it until it closes.
async fn publish_camera(connection: &Arc<RTCPeerConnection>) {
    let track = Arc::new(TrackLocalStaticRTP::new(
        RTCRtpCodecCapability { mime_type: MIME_TYPE_VP8.to_owned(), ..Default::default() },
        "camera".to_owned(),
        "local".to_owned(),
    ));
    connection.add_track(track.clone() as Arc<dyn TrackLocal + Send + Sync>).await.unwrap();
    let connection = connection.clone();
    tokio::spawn(async move {
        let mut sequence_number = 0u16;
        while connection.connection_state() != RTCPeerConnectionState::Closed {
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
