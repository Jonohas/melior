//! Value objects exchanged while negotiating a media connection.

/// A session description (offer or answer body). Opaque to the domain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sdp(String);

impl Sdp {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

/// A trickled ICE candidate.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IceCandidate {
    pub candidate: String,
    pub sdp_mid: Option<String>,
    pub sdp_mline_index: Option<u16>,
    pub username_fragment: Option<String>,
}

/// A STUN/TURN server participants may use to reach the SFU.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IceServer {
    pub urls: Vec<String>,
    pub username: Option<String>,
    pub credential: Option<String>,
}
