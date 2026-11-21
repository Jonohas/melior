//! The Melior use cases and the ports they depend on, one module per bounded context.
//!
//! - [`chat`]: channel chat and its history.
//! - [`communities`]: communities, their channels, roles and invites, admission
//!   to channels, the voice roster and the community feed.
//! - [`conference`]: joining rooms and negotiating who receives which tracks.
//! - [`auth`]: the login round trip and server-side sessions.
//!
//! Invariants of this crate:
//!
//! - It names no concrete adapter. Everything outside the process (media, the
//!   identity provider, session storage, the client connection) is a trait in a
//!   `ports` module, implemented in `melior-infrastructure` or
//!   `melior-interfaces`.
//! - It depends on no web framework, WebRTC stack or storage client
//!   (`cargo xtask layers` checks this).
//! - No function in it panics.

#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

pub mod auth;
pub mod chat;
pub mod communities;
pub mod conference;

pub use auth::{
    ActiveSession, AuthError, AuthService, AuthSettings, Authentication, AuthorizationRequest,
    CompletedLogin, IdentityProvider, IdpError, InMemorySessionStore, LoginRedirect, SessionState,
    SessionStore, StoreError, StoredSession, Tokens,
};
pub use chat::{
    ChatFailure, ChatPage, ChatRepository, ChatService, ChatStoreError, InMemoryChatRepository,
};
pub use communities::{
    ChannelGrant, ChannelView, CommunitiesError, CommunityEvent, CommunityEvents,
    CommunityRepository, CommunityService, CommunityStoreError, CommunityView, EventsError,
    FeedEnd, FeedFilter, FeedVerdict, InMemoryCommunityEvents, InMemoryCommunityRepository,
    InMemoryVoiceRoster, InvitePreview, MemberView, PresenceQueue, Redemption, RemovalReason,
    VoiceGrant, VoiceRoster,
};
pub use conference::{
    Admission, ConferenceError, ConferenceService, IceServerProvider, LeaseError, MediaEngine,
    MediaError, MediaEvents, MediaOffer, MediaSession, NoPresence, PresenceContext, PresenceSink,
    PresenceUpdate, RoomLeases, Signal, SignalSink, SingleInstance, StaticIceServers, TrackOwner,
    VoiceEntry,
};
