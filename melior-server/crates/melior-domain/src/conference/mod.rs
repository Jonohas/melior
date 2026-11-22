//! The conference model.

mod effects;
mod error;
mod ids;
mod key_envelope;
mod links;
mod media;
mod mesh;
mod negotiation;
mod plan;
mod room;
mod room_mesh;
#[cfg(test)]
mod room_mesh_tests;
mod room_voice;
#[cfg(test)]
mod room_voice_tests;
mod signaling;
mod simulcast;
mod sync;
mod voice;

pub use effects::{Effects, LinkClose, LinkTimeout, RouteNotice};
pub use error::DomainError;
pub use ids::{ChannelId, SessionId, TrackId};
pub use key_envelope::{KeyEnvelope, MAX_KEY_ENVELOPE_BYTES};
pub use links::{LinkEvent, Links, Pair, Route};
pub use media::{MediaKind, Track, TrackSource};
pub use mesh::{MediaPath, MeshPolicy, RoomTarget};
pub use negotiation::{ClientOffer, Negotiation, NegotiationPhase};
pub use plan::{ChannelMedia, MediaMode, MediaWarning, RoomPlan};
pub use room::{Departure, Joined, ParticipantInfo, Room};
pub use signaling::{IceCandidate, IceServer, Sdp};
pub use simulcast::{ReceiverLayer, SimulcastLayer};
pub use sync::{SectionChoice, SectionView, StreamLabel, SyncGroup, choose_section};
pub use voice::{VoiceLimit, VoiceState};
