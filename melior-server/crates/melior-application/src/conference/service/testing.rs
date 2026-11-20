//! Fakes for the ports, shared by the conference tests.

use super::*;
use crate::conference::{
    Admission, MediaOffer, PresenceSink, PresenceUpdate, SingleInstance, StaticIceServers,
};
use melior_domain::{ChannelMedia, MediaKind, UserId};

/// Records what the application asks of the media plane.
#[derive(Default)]
pub(super) struct FakeEngine {
    pub(super) sessions: Mutex<HashMap<SessionId, Arc<FakeSession>>>,
}

impl FakeEngine {
    pub(super) fn session(&self, id: &SessionId) -> Arc<FakeSession> {
        lock(&self.sessions).get(id).cloned().expect("media session")
    }
}

#[async_trait]
impl MediaEngine for FakeEngine {
    async fn create_session(
        &self,
        session_id: SessionId,
        _events: Weak<dyn MediaEvents>,
    ) -> Result<Arc<dyn MediaSession>, MediaError> {
        let session = Arc::new(FakeSession::default());
        lock(&self.sessions).insert(session_id, session.clone());
        Ok(session)
    }
}

#[derive(Default)]
pub(super) struct FakeSession {
    pub(super) subscribed: Mutex<Vec<TrackId>>,
    pub(super) offers_created: Mutex<usize>,
    pub(super) closed: Mutex<bool>,
    pub(super) reject_answers: Mutex<bool>,
}

impl FakeSession {
    pub(super) fn subscribed(&self) -> Vec<TrackId> {
        let mut tracks = lock(&self.subscribed).clone();
        tracks.sort();
        tracks
    }
}

#[async_trait]
impl MediaSession for FakeSession {
    async fn apply_offer(&self, _offer: Sdp) -> Result<Sdp, MediaError> {
        Ok(Sdp::new("answer"))
    }

    async fn create_offer(&self) -> Result<MediaOffer, MediaError> {
        let mut count = lock(&self.offers_created);
        *count += 1;
        // One media section per subscription, named after its position.
        let carrying = lock(&self.subscribed)
            .iter()
            .enumerate()
            .map(|(index, track)| (index.to_string(), track.clone()))
            .collect();
        Ok(MediaOffer { sdp: Sdp::new(format!("offer-{count}")), carrying })
    }

    async fn apply_answer(&self, _answer: Sdp) -> Result<(), MediaError> {
        if *lock(&self.reject_answers) {
            return Err(MediaError::Backend("bad answer".into()));
        }
        Ok(())
    }

    async fn add_ice_candidate(&self, _candidate: IceCandidate) -> Result<(), MediaError> {
        Ok(())
    }

    async fn subscribe(&self, track: &TrackId) -> Result<(), MediaError> {
        lock(&self.subscribed).push(track.clone());
        Ok(())
    }

    async fn unsubscribe(&self, track: &TrackId) -> Result<(), MediaError> {
        lock(&self.subscribed).retain(|t| t != track);
        Ok(())
    }

    async fn close(&self) {
        *lock(&self.closed) = true;
    }
}

#[derive(Default)]
pub(super) struct RecordingSink {
    pub(super) signals: Mutex<Vec<Signal>>,
}

impl RecordingSink {
    pub(super) fn take_all(&self) -> Vec<Signal> {
        std::mem::take(&mut lock(&self.signals))
    }

    /// The signals so far, minus roster announcements and track ownership
    /// (most tests are about negotiation and would only be cluttered by them).
    pub(super) fn take(&self) -> Vec<Signal> {
        self.take_all()
            .into_iter()
            .filter(|signal| !matches!(signal, Signal::PeerJoined(_)))
            .map(|signal| match signal {
                Signal::Offer { sdp, .. } => Signal::Offer { sdp, tracks: Vec::new() },
                other => other,
            })
            .collect()
    }
}

/// An offer as [`RecordingSink::take`] reports it.
pub(super) fn offer(sdp: &str) -> Signal {
    Signal::Offer { sdp: Sdp::new(sdp), tracks: Vec::new() }
}

pub(super) fn member(username: &str) -> User {
    User::new(
        UserId::new(format!("sub-{username}")),
        username,
        None,
        [User::ROLE_MEMBER.to_owned()],
    )
}

impl SignalSink for RecordingSink {
    fn send(&self, signal: Signal) {
        lock(&self.signals).push(signal);
    }
}

pub(super) struct Client {
    pub(super) id: SessionId,
    pub(super) sink: Arc<RecordingSink>,
    pub(super) media: Arc<FakeSession>,
}

/// Records the presence reports.
#[derive(Default)]
pub(super) struct RecordingPresence(pub(super) Mutex<Vec<PresenceUpdate>>);

impl RecordingPresence {
    pub(super) fn take(&self) -> Vec<PresenceUpdate> {
        std::mem::take(&mut lock(&self.0))
    }
}

impl PresenceSink for RecordingPresence {
    fn report(&self, update: PresenceUpdate) {
        lock(&self.0).push(update);
    }
}

pub(super) struct Harness {
    pub(super) engine: Arc<FakeEngine>,
    pub(super) service: Arc<ConferenceService>,
    pub(super) presence: Arc<RecordingPresence>,
}

impl Harness {
    pub(super) fn new() -> Self {
        Self::with_mesh(MeshPolicy::OFF)
    }

    pub(super) fn with_mesh(mesh: MeshPolicy) -> Self {
        let engine = Arc::new(FakeEngine::default());
        let presence = Arc::new(RecordingPresence::default());
        let service = ConferenceService::new(
            engine.clone(),
            Arc::new(StaticIceServers::default()),
            Arc::new(SingleInstance),
            mesh,
            presence.clone(),
        );
        Self { engine, service, presence }
    }

    /// An `Auto` channel without a limit, outside any community.
    pub(super) fn admission(&self, channel: &str) -> Admission {
        let plan = RoomPlan::resolve(ChannelMedia::default(), self.service.mesh());
        Admission::unlisted(ChannelId::parse(channel).unwrap(), plan)
    }

    pub(super) async fn join(&self, channel: &str) -> Client {
        let sink = Arc::new(RecordingSink::default());
        let id = self
            .service
            .join(self.admission(channel), &member("someone"), sink.clone())
            .await
            .unwrap();
        let media = self.engine.session(&id);
        Client { id, sink, media }
    }

    /// Joins, completes the first offer and connects, discarding the signals so far.
    pub(super) async fn join_ready(&self, channel: &str) -> Client {
        let client = self.join(channel).await;
        self.service.handle_offer(&client.id, Sdp::new("offer")).await.unwrap();
        self.service.connected(&client.id).await;
        client.sink.take();
        client
    }

    /// Like [`Self::join_ready`], but keeps the signals: the ones sent while joining
    /// (routes, uplinks) are what mesh tests look at.
    pub(super) async fn join_ready_keeping(&self, channel: &str) -> Client {
        let client = self.join(channel).await;
        self.service.handle_offer(&client.id, Sdp::new("offer")).await.unwrap();
        self.service.connected(&client.id).await;
        client
    }

    pub(super) async fn publish(&self, client: &Client) -> Track {
        let track = Track::new(TrackId::generate(), MediaKind::Video);
        self.service.track_published(&client.id, track.clone()).await;
        track
    }
}
