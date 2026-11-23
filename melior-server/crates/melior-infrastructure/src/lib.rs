//! Adapters for the application's ports, and other technical concerns.
//!
//! - [`webrtc`]: the media plane (`MediaEngine`, `MediaSession`) on webrtc-rs.
//! - [`oidc`]: the identity provider (`IdentityProvider`) over OpenID Connect.
//! - [`redis`]: the session store (`SessionStore`), the room leases (`RoomLeases`),
//!   the community event bus (`CommunityEvents`) and the voice roster (`VoiceRoster`).
//! - [`postgres`]: communities (`CommunityRepository`), the chat history and image records
//!   (`ChatRepository`).
//! - [`s3`]: the image store (`ImageStore`) on S3-compatible object storage (`RustFS`).
//! - [`telemetry`]: logging and OpenTelemetry trace export.
//! - [`turn`]: per-user, expiring TURN credentials (`IceServerProvider`).
//! - [`config`]: runtime configuration from environment variables.
//!
//! Invariants of this crate:
//!
//! - It implements ports; it holds no business rule. A decision about rooms,
//!   negotiation, roles or sessions belongs in `melior-domain` or
//!   `melior-application`.
//! - It does not depend on `melior-interfaces` or on a web framework
//!   (`cargo xtask layers` checks this).
//! - Nothing received from the network is trusted before it is verified, and no
//!   function in it panics on such input.

#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

pub mod config;
pub mod oidc;
pub mod postgres;
pub mod redis;
pub mod s3;
pub mod telemetry;
pub mod turn;
pub mod webrtc;
