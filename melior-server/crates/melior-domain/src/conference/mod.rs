//! The conference model.

mod access;
mod effects;
mod error;
mod ids;
mod links;
mod media;
mod mesh;
mod negotiation;
mod room;
mod room_mesh;
#[cfg(test)]
mod room_mesh_tests;
mod signaling;

pub use access::RoomAccess;
pub use effects::{Effects, LinkClose, LinkTimeout, RouteNotice};
pub use error::DomainError;
pub use ids::{ChannelId, SessionId, TrackId};
pub use links::{LinkEvent, Links, Pair, Route};
pub use media::{MediaKind, Track, TrackSource};
pub use mesh::{MediaPath, MeshPolicy, RoomTarget};
pub use negotiation::{ClientOffer, Negotiation, NegotiationPhase};
pub use room::{Departure, Joined, ParticipantInfo, Room};
pub use signaling::{IceCandidate, IceServer, Sdp};
