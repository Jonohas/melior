//! What the use cases need from the outside world. Infrastructure and interface
//! adapters implement these traits; the application never names a concrete adapter.

use async_trait::async_trait;
use melior_domain::{
    ChannelId, ChatMessage, IceCandidate, IceServer, KeyEnvelope, MediaPath, ParticipantInfo, Sdp,
    SessionId, SyncGroup, Track, TrackId, TrackSource, User, VoiceState,
};
use std::sync::{Arc, Weak};

#[derive(thiserror::Error, Debug)]
pub enum MediaError {
    #[error("track {0} is not available")]
    TrackNotFound(TrackId),

    #[error("media backend: {0}")]
    Backend(String),
}

/// The media plane: creates one media session per participant.
#[async_trait]
pub trait MediaEngine: Send + Sync {
    /// Opens a media session. Things the session observes later (published tracks,
    /// local ICE candidates) are reported to `events`.
    async fn create_session(
        &self,
        session_id: SessionId,
        events: Weak<dyn MediaEvents>,
    ) -> Result<Arc<dyn MediaSession>, MediaError>;
}

/// One participant's media connection to the SFU.
#[async_trait]
pub trait MediaSession: Send + Sync {
    /// Applies the client's offer and returns the answer.
    async fn apply_offer(&self, offer: Sdp) -> Result<Sdp, MediaError>;

    /// Produces an offer describing the tracks currently sent to the client.
    async fn create_offer(&self) -> Result<MediaOffer, MediaError>;

    /// Applies the client's answer to the last offer from [`Self::create_offer`].
    async fn apply_answer(&self, answer: Sdp) -> Result<(), MediaError>;

    async fn add_ice_candidate(&self, candidate: IceCandidate) -> Result<(), MediaError>;

    /// Starts sending a published track to this participant (after the next negotiation).
    /// The participant's browser plays it in step with the other tracks of `group`, and
    /// with no others.
    async fn subscribe(&self, track: &TrackId, group: &SyncGroup) -> Result<(), MediaError>;

    /// Stops sending a track to this participant.
    async fn unsubscribe(&self, track: &TrackId) -> Result<(), MediaError>;

    async fn close(&self);
}

/// An offer from the media plane, with what each of its sending media sections carries.
///
/// A media section outlives the track it was opened for and is reused for other
/// tracks, and nothing in the session description may change to say so. The
/// mapping therefore travels next to the description.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaOffer {
    pub sdp: Sdp,
    /// `(mid of the media section, track it carries)` for every section in use.
    pub carrying: Vec<(String, TrackId)>,
}

/// Tells a client whose track a media section carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackOwner {
    /// The media section's mid.
    pub mid: String,
    /// The participant who publishes the track.
    pub session_id: SessionId,
    /// What the track shows: the participant's camera or its screen.
    pub source: TrackSource,
}

/// Events the media plane raises towards the application.
#[async_trait]
pub trait MediaEvents: Send + Sync {
    /// The session's media connection is established (possibly again, after a drop).
    async fn connected(&self, session_id: &SessionId);

    async fn track_published(&self, session_id: &SessionId, track: Track);

    async fn track_ended(&self, session_id: &SessionId, track_id: &TrackId);

    fn ice_candidate(&self, session_id: &SessionId, candidate: IceCandidate);
}

/// A message for a participant's client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Signal {
    /// Sent once, first: who the participant is and who is already in the room.
    Joined {
        session_id: SessionId,
        channel_id: ChannelId,
        username: String,
        ice_servers: Vec<IceServer>,
        participants: Vec<ParticipantInfo>,
    },
    PeerJoined(ParticipantInfo),
    Answer(Sdp),
    /// A renegotiation, with the complete list of which media section carries
    /// whose track from now on.
    Offer {
        sdp: Sdp,
        tracks: Vec<TrackOwner>,
    },
    IceCandidate(IceCandidate),
    PeerLeft(SessionId),
    /// Play `peer`'s media over `path` as soon as it works there, then report
    /// `route_live` with this `epoch`.
    Route {
        peer: SessionId,
        path: MediaPath,
        epoch: u64,
    },
    /// Close the direct connection to `peer`.
    LinkClose {
        peer: SessionId,
    },
    /// Pause (`false`) or resume (`true`) sending on the SFU connection.
    Uplink {
        active: bool,
    },
    /// An offer for the direct connection with `from`, relayed by the server.
    PeerOffer {
        from: SessionId,
        sdp: Sdp,
    },
    /// An answer for the direct connection with `from`.
    PeerAnswer {
        from: SessionId,
        sdp: Sdp,
    },
    /// An ICE candidate for the direct connection with `from`.
    PeerIce {
        from: SessionId,
        candidate: IceCandidate,
    },
    /// An end-to-end key message from `from`, relayed unread.
    Key {
        from: SessionId,
        envelope: KeyEnvelope,
    },
    /// A participant's voice state changed; everyone in the room hears it, the
    /// participant included.
    VoiceState {
        session_id: SessionId,
        voice: VoiceState,
    },
    /// Someone said something in the room. `client_id` is set only for the
    /// author, so its client can tell which of its pending messages this is.
    Chat {
        message: ChatMessage,
        client_id: Option<String>,
    },
    Error(String),
}

/// Delivers signals to one participant's client. Must not block.
pub trait SignalSink: Send + Sync {
    fn send(&self, signal: Signal);
}

/// Decides which ICE servers a participant is told to use. Credentials, if any,
/// are issued per user and expire, so a leaked message is of limited use.
pub trait IceServerProvider: Send + Sync {
    fn ice_servers_for(&self, user: &User) -> Vec<IceServer>;
}

#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
#[error("room leases: {0}")]
pub struct LeaseError(pub String);

/// Which server instance hosts a room.
///
/// A room's media lives in the memory of one instance. With several instances
/// behind one address, a lease per room makes sure the instances agree on which
/// of them that is. A lease is never given back: it expires unless it is renewed,
/// which covers both a room that emptied and an instance that died, and leaves
/// no moment at which a room that is filling up again has no lease.
#[async_trait]
pub trait RoomLeases: Send + Sync {
    /// Takes or renews this instance's lease on a room. Returns false if another
    /// instance holds it.
    async fn hold(&self, channel_id: &ChannelId) -> Result<bool, LeaseError>;
}

/// [`RoomLeases`] for a deployment with a single instance: it hosts every room.
#[derive(Debug, Default, Clone, Copy)]
pub struct SingleInstance;

#[async_trait]
impl RoomLeases for SingleInstance {
    async fn hold(&self, _channel_id: &ChannelId) -> Result<bool, LeaseError> {
        Ok(true)
    }
}

/// [`IceServerProvider`] that hands every participant the same servers.
#[derive(Debug, Default, Clone)]
pub struct StaticIceServers(pub Vec<IceServer>);

impl IceServerProvider for StaticIceServers {
    fn ice_servers_for(&self, _user: &User) -> Vec<IceServer> {
        self.0.clone()
    }
}
