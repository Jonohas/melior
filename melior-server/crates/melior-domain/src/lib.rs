//! The Melior domain model, one module per bounded context.
//!
//! - [`chat`]: chat messages and the rules on what may be said.
//! - [`community`]: communities, their roles, channels and invites, and who may
//!   do what where.
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
pub mod community;
pub mod conference;
pub mod identity;

pub use chat::{
    Attachment, AttachmentId, AttachmentRequest, ChatContent, ChatError, ChatMessage, ChatText,
    GifLink, ImageFormat, MessageId, SendLimit, StoredImage,
};
pub use community::{
    Category, CategoryChanges, CategoryId, Channel, ChannelChanges, ChannelDraft, ChannelKind,
    Community, CommunityError, CommunityId, CommunityState, Invite, InviteCode, InviteSettings,
    Member, Name, Override, OverrideTarget, Permissions, Rank, Refusal, Rejected, Role,
    RoleChanges, RoleDraft, RoleId, Standing, Topic,
};
pub use conference::{
    ChannelId, ChannelMedia, ClientOffer, Departure, DomainError, Effects, IceCandidate, IceServer,
    Joined, KeyEnvelope, LinkClose, LinkTimeout, MAX_KEY_ENVELOPE_BYTES, MediaKind, MediaMode,
    MediaPath, MediaWarning, MeshPolicy, Negotiation, NegotiationPhase, Pair, ParticipantInfo,
    ReceiverLayer, Room, RoomPlan, RoomTarget, Route, RouteNotice, Sdp, SectionChoice, SectionView,
    SessionId, SimulcastLayer, StreamLabel, SyncGroup, Track, TrackId, TrackSource, VoiceLimit,
    VoiceState, choose_section,
};
pub use identity::{AuthSessionId, ReturnPath, User, UserId, Username};
