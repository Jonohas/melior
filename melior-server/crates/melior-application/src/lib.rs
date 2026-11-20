//! The Melior use cases and the ports they depend on, one module per bounded context.
//!
//! - [`chat`]: room chat and its history.
//! - [`conference`]: joining rooms and negotiating who receives which tracks.
//! - [`auth`]: the login round trip and server-side sessions.
//! - [`rooms`]: the room registry: who owns a room and who may enter it.
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
pub mod conference;
pub mod rooms;

pub use auth::{
    ActiveSession, AuthError, AuthService, AuthSettings, Authentication, AuthorizationRequest,
    CompletedLogin, IdentityProvider, IdpError, InMemorySessionStore, LoginRedirect, SessionState,
    SessionStore, StoreError, StoredSession, Tokens,
};
pub use chat::{
    ChatFailure, ChatPage, ChatRepository, ChatService, ChatStoreError, InMemoryChatRepository,
};
pub use conference::{
    ConferenceError, ConferenceService, IceServerProvider, LeaseError, MediaEngine, MediaError,
    MediaEvents, MediaOffer, MediaSession, RoomLeases, Signal, SignalSink, SingleInstance,
    StaticIceServers, TrackOwner,
};
pub use rooms::{
    InMemoryRoomRepository, RepositoryError, RoomAccessService, RoomRepository, RoomsError,
};
