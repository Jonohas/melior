//! The Melior domain model, one module per bounded context.
//!
//! - [`chat`]: chat messages and the rules on what may be said.
//! - [`conference`]: rooms, participants, tracks and the negotiation rules.
//! - [`identity`]: who a user is and what they may do.
//!
//! Invariants of this crate:
//!
//! - It is pure and synchronous. It performs no I/O and depends on no async
//!   runtime, web framework, WebRTC stack or storage client
//!   (`cargo xtask layers` checks this).
//! - Its methods decide; they do not act. A method returns what has to happen
//!   on the media plane or towards clients, and the application layer does it.
//! - No function in it panics.

#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

pub mod chat;
pub mod conference;
pub mod identity;

pub use chat::{ChatError, ChatMessage, ChatText, MessageId, SendLimit};
pub use conference::{
    ChannelId, ClientOffer, Departure, DomainError, Effects, IceCandidate, IceServer, Joined,
    LinkClose, LinkTimeout, MediaKind, MediaPath, MeshPolicy, Negotiation, NegotiationPhase, Pair,
    ParticipantInfo, Room, RoomAccess, RoomTarget, Route, RouteNotice, Sdp, SessionId, Track,
    TrackId, TrackSource,
};
pub use identity::{AuthSessionId, ReturnPath, User, UserId, Username};
