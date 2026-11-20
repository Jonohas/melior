use super::ports::{
    IceServerProvider, LeaseError, MediaEngine, MediaError, MediaEvents, MediaSession, RoomLeases,
    Signal, SignalSink, TrackOwner,
};
use async_trait::async_trait;
use melior_domain::{
    ChannelId, DomainError, IceCandidate, MeshPolicy, Negotiation, ParticipantInfo, Room, Sdp,
    SessionId, Track, TrackId, User,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use tokio::sync::Mutex as AsyncMutex;
use tracing::{debug, info, warn};

mod mesh;

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
    mesh: MeshPolicy,
    this: Weak<ConferenceService>,
}

impl ConferenceService {
    pub fn new(
        media: Arc<dyn MediaEngine>,
        ice_servers: Arc<dyn IceServerProvider>,
        leases: Arc<dyn RoomLeases>,
        mesh: MeshPolicy,
    ) -> Arc<Self> {
        Arc::new_cyclic(|this| Self {
            media,
            ice_servers,
            leases,
            rooms: Mutex::new(HashMap::new()),
            sessions: Mutex::new(HashMap::new()),
            mesh,
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

        let joined = match room.join(session_id.clone(), user.username.clone()) {
            Ok(joined) => joined,
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
            participants: joined.present.clone(),
        });
        let newcomer =
            ParticipantInfo { session_id: session_id.clone(), username: user.username.clone() };
        for other in joined.present {
            if let Some(other) = self.session(&other.session_id) {
                other.signals.send(Signal::PeerJoined(newcomer.clone()));
            }
        }
        self.carry_out(&mut room, joined.effects).await;
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
                    self.carry_out(&mut room, departure.effects).await;
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
            .or_insert_with(|| {
                Arc::new(AsyncMutex::new(Room::with_mesh(channel_id.clone(), self.mesh)))
            })
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
mod mesh_tests;
#[cfg(test)]
mod testing;
#[cfg(test)]
mod tests;
