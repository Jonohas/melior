//! The media plane, implemented with webrtc-rs.

mod engine;
mod forwarder;
mod session;

pub use engine::WebRtcMediaEngine;

use melior_application::MediaError;

/// Translates a webrtc-rs error into the application's vocabulary. By value, so
/// it fits `map_err`.
#[allow(clippy::needless_pass_by_value)]
fn backend(error: webrtc::Error) -> MediaError {
    MediaError::Backend(error.to_string())
}
