use super::ports::{
    IceServerProvider, LeaseError, MediaEngine, MediaError, MediaEvents, MediaSession, RoomLeases,
    Signal, SignalSink, TrackOwner,
};
use async_trait::async_trait;
use melior_domain::{
    ChannelId, DomainError, IceCandidate, Negotiation, ParticipantInfo, Room, Sdp, SessionId,
    Track, TrackId, User,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use tokio::sync::Mutex as AsyncMutex;
use tracing::{debug, info, warn};

#[derive(thiserror::Error, Debug)]
pub enum ConferenceError {
    #[error("unknown session")]
    UnknownSession,

    #[error("user {0} is not allowed to join rooms")]
    NotAllowed(String),

    /// Another server instance hosts this room. The client should try again: its
    /// connection will be routed there, or here once that instance is gone.
    #[error("this room is hosted by another server instance")]
    HostedElsewhere,

    #[error(transparent)]
    Lease(#[from] LeaseError),

    #[error(transparent)]
    Domain(#[from] DomainError),

    #[error(transparent)]
    Media(#[from] MediaError),
}

/// A room and the lock that makes each use case on it atomic.
///
/// Every change to a room (and the media operations that follow from it) runs with this
/// lock held, so rooms are independent of each other and a room never observes a
/// half-applied change. Nothing awaited under the lock waits on a client: signals are
/// queued, and answers arrive as separate use cases.
type RoomCell = Arc<AsyncMutex<Room>>;

/// A joined participant as the application sees it: its room plus its two adapters.
struct Session {
    id: SessionId,
    channel_id: ChannelId,
    room: RoomCell,
    media: Arc<dyn MediaSession>,
    signals: Arc<dyn SignalSink>,
}

/// The conference use cases: joining and leaving rooms, and negotiating who receives
/// which tracks.
pub struct ConferenceService {
    media: Arc<dyn MediaEngine>,
    ice_servers: Arc<dyn IceServerProvider>,
    leases: Arc<dyn RoomLeases>,
    rooms: Mutex<HashMap<ChannelId, RoomCell>>,
    sessions: Mutex<HashMap<SessionId, Arc<Session>>>,
    this: Weak<ConferenceService>,
}

impl ConferenceService {
    pub fn new(
        media: Arc<dyn MediaEngine>,
        ice_servers: Arc<dyn IceServerProvider>,
        leases: Arc<dyn RoomLeases>,
    ) -> Arc<Self> {
        Arc::new_cyclic(|this| Self {
            media,
            ice_servers,
            leases,
            rooms: Mutex::new(HashMap::new()),
            sessions: Mutex::new(HashMap::new()),
            this: this.clone(),
        })
    }

    /// Makes sure this instance is the one hosting a room, taking or renewing its
    /// lease. Called before a join and periodically while participants are
    /// connected; [`ConferenceError::HostedElsewhere`] means they must reconnect.
    pub async fn host(&self, channel_id: &ChannelId) -> Result<(), ConferenceError> {
        if self.leases.hold(channel_id).await? {
            Ok(())
        } else {
            Err(ConferenceError::HostedElsewhere)
        }
    }

    /// Adds a user to a channel as a new participant, tells its client who it is and
    /// who is there, and announces it to the others.
    #[tracing::instrument(name = "conference.join", skip_all, fields(channel = %channel_id, user = %user.username))]
    pub async fn join(
        &self,
        channel_id: ChannelId,
        user: &User,
        signals: Arc<dyn SignalSink>,
    ) -> Result<SessionId, ConferenceError> {
        if !user.may_join_rooms() {
            return Err(ConferenceError::NotAllowed(user.username.clone()));
        }
        let session_id = SessionId::generate();
        let events: Weak<dyn MediaEvents> = self.this.clone();
        let media = self.media.create_session(session_id.clone(), events).await?;

        let (cell, mut room) = loop {
            let cell = self.room_cell(&channel_id);
            let room = cell.clone().lock_owned().await;
            // The room may have emptied and been dropped while we waited for it.
            if self.is_current_room(&channel_id, &cell) {
                break (cell, room);
            }
        };

        let present = match room.join(session_id.clone(), user.username.clone()) {
            Ok(present) => present,
            Err(error) => {
                drop(room);
                media.close().await;
                return Err(error.into());
            }
        };

        let session = Arc::new(Session {
            id: session_id.clone(),
            channel_id: channel_id.clone(),
            room: cell,
            media,
            signals,
        });
        lock(&self.sessions).insert(session_id.clone(), session.clone());

        info!(session = %session_id, channel = %channel_id, user = %user.username, "participant joined");
        // Still under the room lock, so the newcomer hears about itself before
        // anything else, and the others hear about it before its tracks.
        session.signals.send(Signal::Joined {
            session_id: session_id.clone(),
            channel_id,
            username: user.username.clone(),
            ice_servers: self.ice_servers.ice_servers_for(user),
            participants: present.clone(),
        });
        let newcomer =
            ParticipantInfo { session_id: session_id.clone(), username: user.username.clone() };
        for other in present {
            if let Some(other) = self.session(&other.session_id) {
                other.signals.send(Signal::PeerJoined(newcomer.clone()));
            }
        }
        Ok(session_id)
    }

    /// Removes a participant, stops forwarding its tracks to the others and releases
    /// its media session. Safe to call more than once.
    #[tracing::instrument(name = "conference.leave", skip_all, fields(session = %session_id))]
    pub async fn leave(&self, session_id: &SessionId) {
        let Some(session) = lock(&self.sessions).remove(session_id) else {
            return;
        };

        {
            let mut room = session.room.lock().await;
            match room.leave(session_id) {
                Ok(departure) => {
                    for remaining in &departure.remaining {
                        if let Some(other) = self.session(remaining) {
                            other.signals.send(Signal::PeerLeft(session_id.clone()));
                        }
                    }
                    for (subscriber_id, track_ids) in departure.unsubscribed {
                        let Some(subscriber) = self.session(&subscriber_id) else {
                            continue;
                        };
                        for track_id in &track_ids {
                            if let Err(error) = subscriber.media.unsubscribe(track_id).await {
                                warn!(session = %subscriber_id, track = %track_id, %error, "unsubscribe failed");
                            }
                        }
                        self.renegotiate(&mut room, &subscriber).await;
                    }
                }
                Err(error) => warn!(session = %session_id, %error, "leave"),
            }

            if room.is_empty() {
                let mut rooms = lock(&self.rooms);
                if rooms
                    .get(&session.channel_id)
                    .is_some_and(|cell| Arc::ptr_eq(cell, &session.room))
                {
                    rooms.remove(&session.channel_id);
                }
            }
        }

        session.media.close().await;
        info!(session = %session_id, channel = %session.channel_id, "participant left");
    }

    /// Answers a client offer. The participant starts receiving the others' tracks
    /// once its media connection is up (see [`MediaEvents::connected`]).
    #[tracing::instrument(name = "conference.offer", skip_all, fields(session = %session_id))]
    pub async fn handle_offer(
        &self,
        session_id: &SessionId,
        offer: Sdp,
    ) -> Result<(), ConferenceError> {
        let session = self.require_session(session_id)?;
        let mut room = session.room.lock().await;

        room.negotiation(session_id)?.ensure_can_accept_offer()?;
        let answer = session.media.apply_offer(offer).await?;
        room.client_offer_answered(session_id)?;
        session.signals.send(Signal::Answer(answer));
        Ok(())
    }

    /// Applies the client's answer to a server offer, then negotiates again if the
    /// participant's tracks changed in the meantime.
    #[tracing::instrument(name = "conference.answer", skip_all, fields(session = %session_id))]
    pub async fn handle_answer(
        &self,
        session_id: &SessionId,
        answer: Sdp,
    ) -> Result<(), ConferenceError> {
        let session = self.require_session(session_id)?;
        let mut room = session.room.lock().await;

        room.negotiation(session_id)?.ensure_can_accept_answer()?;
        match session.media.apply_answer(answer).await {
            Ok(()) => {
                if room.negotiation_mut(session_id)?.answer_applied() {
                    self.renegotiate(&mut room, &session).await;
                }
                Ok(())
            }
            Err(error) => {
                room.negotiation_mut(session_id)?.answer_rejected();
                Err(error.into())
            }
        }
    }

    pub async fn add_ice_candidate(
        &self,
        session_id: &SessionId,
        candidate: IceCandidate,
    ) -> Result<(), ConferenceError> {
        let session = self.require_session(session_id)?;
        session.media.add_ice_candidate(candidate).await?;
        Ok(())
    }

    /// The room a participant is in and the username it joined with.
    pub async fn whereabouts(
        &self,
        session_id: &SessionId,
    ) -> Result<(ChannelId, String), ConferenceError> {
        let session = self.require_session(session_id)?;
        let room = session.room.lock().await;
        let username = room
            .roster()
            .into_iter()
            .find(|participant| &participant.session_id == session_id)
            .map(|participant| participant.username)
            .ok_or(ConferenceError::UnknownSession)?;
        Ok((session.channel_id.clone(), username))
    }

    /// Tells everyone in a participant's room something: `for_sender` to the
    /// participant, `for_others` to the rest. Runs under the room lock, so the
    /// room's participants hear it in the same order as its other signals.
    pub async fn tell_room(
        &self,
        session_id: &SessionId,
        for_sender: Signal,
        for_others: Signal,
    ) -> Result<(), ConferenceError> {
        let session = self.require_session(session_id)?;
        let room = session.room.lock().await;
        for participant in room.roster() {
            let Some(target) = self.session(&participant.session_id) else {
                continue;
            };
            let signal = if &participant.session_id == session_id {
                for_sender.clone()
            } else {
                for_others.clone()
            };
            target.signals.send(signal);
        }
        Ok(())
    }

    /// Sends the participant a fresh offer if its negotiation allows one right now;
    /// otherwise the negotiation remembers that one is due.
    async fn renegotiate(&self, room: &mut Room, session: &Session) {
        let wanted = room.negotiation_mut(&session.id).is_ok_and(Negotiation::request_offer);
        if !wanted {
            return;
        }
        match session.media.create_offer().await {
            Ok(offer) => {
                // A section whose track ended since the offer was made carries
                // nothing the client could show, so it is left out.
                let tracks = offer
                    .carrying
                    .into_iter()
                    .filter_map(|(mid, track_id)| {
                        let session_id = room.publisher_of(&track_id)?.clone();
                        let source = room.source_of(&track_id)?;
                        Some(TrackOwner { mid, session_id, source })
                    })
                    .collect();
                session.signals.send(Signal::Offer { sdp: offer.sdp, tracks });
            }
            Err(error) => {
                warn!(session = %session.id, %error, "could not create offer");
                if let Ok(negotiation) = room.negotiation_mut(&session.id) {
                    negotiation.offer_failed();
                }
                session.signals.send(Signal::Error(format!("renegotiation failed: {error}")));
            }
        }
    }

    fn session(&self, id: &SessionId) -> Option<Arc<Session>> {
        lock(&self.sessions).get(id).cloned()
    }

    fn require_session(&self, id: &SessionId) -> Result<Arc<Session>, ConferenceError> {
        self.session(id).ok_or(ConferenceError::UnknownSession)
    }

    fn room_cell(&self, channel_id: &ChannelId) -> RoomCell {
        lock(&self.rooms)
            .entry(channel_id.clone())
            .or_insert_with(|| Arc::new(AsyncMutex::new(Room::new(channel_id.clone()))))
            .clone()
    }

    fn is_current_room(&self, channel_id: &ChannelId, cell: &RoomCell) -> bool {
        lock(&self.rooms).get(channel_id).is_some_and(|current| Arc::ptr_eq(current, cell))
    }

    #[cfg(test)]
    fn room_count(&self) -> usize {
        lock(&self.rooms).len()
    }
}

#[async_trait]
impl MediaEvents for ConferenceService {
    /// A participant's media connection is up: from now on it receives everything
    /// the others publish, starting with what is already there.
    async fn connected(&self, session_id: &SessionId) {
        let Some(session) = self.session(session_id) else {
            return;
        };
        let mut room = session.room.lock().await;

        let existing = match room.media_connected(session_id) {
            Ok(existing) => existing,
            Err(error) => {
                debug!(session = %session_id, %error, "connected ignored");
                return;
            }
        };
        let mut subscribed = false;
        for track in existing {
            match session.media.subscribe(&track.id).await {
                Ok(()) => subscribed = true,
                Err(error) => {
                    warn!(session = %session_id, track = %track.id, %error, "subscribe failed");
                    room.subscription_failed(session_id, &track.id);
                }
            }
        }
        if subscribed {
            self.renegotiate(&mut room, &session).await;
        }
    }

    /// A participant started sending a track: forward it to everyone else who is ready.
    async fn track_published(&self, session_id: &SessionId, track: Track) {
        let Some(session) = self.session(session_id) else {
            return;
        };
        let mut room = session.room.lock().await;

        let subscribers = match room.publish(session_id, track.clone()) {
            Ok(subscribers) => subscribers,
            Err(error) => {
                debug!(session = %session_id, track = %track.id, %error, "publish ignored");
                return;
            }
        };
        info!(session = %session_id, track = %track.id, kind = ?track.kind, subscribers = subscribers.len(), "track published");

        for subscriber_id in subscribers {
            let Some(subscriber) = self.session(&subscriber_id) else {
                continue;
            };
            match subscriber.media.subscribe(&track.id).await {
                Ok(()) => self.renegotiate(&mut room, &subscriber).await,
                Err(error) => {
                    warn!(session = %subscriber_id, track = %track.id, %error, "subscribe failed");
                    room.subscription_failed(&subscriber_id, &track.id);
                }
            }
        }
    }

    /// A published track stopped: stop forwarding it.
    async fn track_ended(&self, session_id: &SessionId, track_id: &TrackId) {
        let Some(session) = self.session(session_id) else {
            return;
        };
        let mut room = session.room.lock().await;

        for subscriber_id in room.unpublish(session_id, track_id) {
            let Some(subscriber) = self.session(&subscriber_id) else {
                continue;
            };
            if let Err(error) = subscriber.media.unsubscribe(track_id).await {
                warn!(session = %subscriber_id, track = %track_id, %error, "unsubscribe failed");
            }
            self.renegotiate(&mut room, &subscriber).await;
        }
    }

    fn ice_candidate(&self, session_id: &SessionId, candidate: IceCandidate) {
        if let Some(session) = self.session(session_id) {
            session.signals.send(Signal::IceCandidate(candidate));
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conference::{MediaOffer, SingleInstance, StaticIceServers};
    use melior_domain::{MediaKind, TrackSource, UserId};

    /// Records what the application asks of the media plane.
    #[derive(Default)]
    struct FakeEngine {
        sessions: Mutex<HashMap<SessionId, Arc<FakeSession>>>,
    }

    impl FakeEngine {
        fn session(&self, id: &SessionId) -> Arc<FakeSession> {
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
    struct FakeSession {
        subscribed: Mutex<Vec<TrackId>>,
        offers_created: Mutex<usize>,
        closed: Mutex<bool>,
        reject_answers: Mutex<bool>,
    }

    impl FakeSession {
        fn subscribed(&self) -> Vec<TrackId> {
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
    struct RecordingSink {
        signals: Mutex<Vec<Signal>>,
    }

    impl RecordingSink {
        fn take_all(&self) -> Vec<Signal> {
            std::mem::take(&mut lock(&self.signals))
        }

        /// The signals so far, minus roster announcements and track ownership
        /// (most tests are about negotiation and would only be cluttered by them).
        fn take(&self) -> Vec<Signal> {
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
    fn offer(sdp: &str) -> Signal {
        Signal::Offer { sdp: Sdp::new(sdp), tracks: Vec::new() }
    }

    fn member(username: &str) -> User {
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

    struct Client {
        id: SessionId,
        sink: Arc<RecordingSink>,
        media: Arc<FakeSession>,
    }

    struct Harness {
        engine: Arc<FakeEngine>,
        service: Arc<ConferenceService>,
    }

    impl Harness {
        fn new() -> Self {
            let engine = Arc::new(FakeEngine::default());
            let service = ConferenceService::new(
                engine.clone(),
                Arc::new(StaticIceServers::default()),
                Arc::new(SingleInstance),
            );
            Self { engine, service }
        }

        async fn join(&self, channel: &str) -> Client {
            let sink = Arc::new(RecordingSink::default());
            let id = self
                .service
                .join(ChannelId::parse(channel).unwrap(), &member("someone"), sink.clone())
                .await
                .unwrap();
            let media = self.engine.session(&id);
            Client { id, sink, media }
        }

        /// Joins, completes the first offer and connects, discarding the signals so far.
        async fn join_ready(&self, channel: &str) -> Client {
            let client = self.join(channel).await;
            self.service.handle_offer(&client.id, Sdp::new("offer")).await.unwrap();
            self.service.connected(&client.id).await;
            client.sink.take();
            client
        }

        async fn publish(&self, client: &Client) -> Track {
            let track = Track::new(TrackId::generate(), MediaKind::Video);
            self.service.track_published(&client.id, track.clone()).await;
            track
        }
    }

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
                TrackOwner {
                    mid: "1".into(),
                    session_id: bob.id.clone(),
                    source: TrackSource::Camera
                },
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
    async fn participants_learn_who_is_in_the_room() {
        let h = Harness::new();
        let alice_sink = Arc::new(RecordingSink::default());
        let alice = h
            .service
            .join(ChannelId::parse("general").unwrap(), &member("alice"), alice_sink.clone())
            .await
            .unwrap();
        alice_sink.take_all();

        let bob_sink = Arc::new(RecordingSink::default());
        let bob = h
            .service
            .join(ChannelId::parse("general").unwrap(), &member("bob"), bob_sink.clone())
            .await
            .unwrap();

        assert!(matches!(
            bob_sink.take_all().as_slice(),
            [Signal::Joined { username, participants, .. }]
                if username == "bob"
                    && *participants == [ParticipantInfo { session_id: alice.clone(), username: "alice".into() }]
        ));
        assert_eq!(
            alice_sink.take_all(),
            vec![Signal::PeerJoined(ParticipantInfo { session_id: bob, username: "bob".into() })]
        );
    }

    #[tokio::test]
    async fn users_without_a_melior_role_cannot_join() {
        let h = Harness::new();
        let guest = User::new(UserId::new("sub-guest"), "guest", None, []);

        let result = h
            .service
            .join(ChannelId::parse("general").unwrap(), &guest, Arc::new(RecordingSink::default()))
            .await;

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
        assert_eq!(
            carol.sink.take(),
            vec![offer("offer-1")],
            "a single offer covering every track"
        );

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
}
