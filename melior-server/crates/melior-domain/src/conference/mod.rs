//! The conference model.

mod access;
mod error;
mod ids;
mod media;
mod negotiation;
mod room;
mod signaling;

pub use access::RoomAccess;
pub use error::DomainError;
pub use ids::{ChannelId, SessionId, TrackId};
pub use media::{MediaKind, Track, TrackSource};
pub use negotiation::{ClientOffer, Negotiation, NegotiationPhase};
pub use room::{Departure, ParticipantInfo, Room};
pub use signaling::{IceCandidate, IceServer, Sdp};
