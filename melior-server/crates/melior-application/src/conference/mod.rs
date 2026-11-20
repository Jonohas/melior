//! Conference use cases: joining rooms and negotiating who receives which tracks.

mod ports;
mod presence;
mod service;

pub use ports::{
    IceServerProvider, LeaseError, MediaEngine, MediaError, MediaEvents, MediaOffer, MediaSession,
    RoomLeases, Signal, SignalSink, SingleInstance, StaticIceServers, TrackOwner,
};
pub use presence::{
    Admission, NoPresence, PresenceContext, PresenceSink, PresenceUpdate, VoiceEntry,
};
pub use service::{ConferenceError, ConferenceService};
