//! The end-to-end key messages participants exchange through the server.
//!
//! The server never reads them: it checks only that one is a short base64url
//! string, and relays it to another participant of the same room. What it carries
//! (public keys, signatures, wrapped media keys) is the clients' business; the
//! server could not decrypt a media key from it.

/// The largest key message the server relays, in bytes of base64url text.
pub const MAX_KEY_ENVELOPE_BYTES: usize = 4096;

/// An opaque, size-bounded base64url key message between two participants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyEnvelope(String);

impl KeyEnvelope {
    /// The message, provided it is 1 to [`MAX_KEY_ENVELOPE_BYTES`] characters of the
    /// base64url alphabet (no padding).
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        let fits = !raw.is_empty() && raw.len() <= MAX_KEY_ENVELOPE_BYTES;
        let base64url =
            raw.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_');
        (fits && base64url).then(|| Self(raw.to_owned()))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_base64url_up_to_the_limit() {
        assert!(KeyEnvelope::parse("eyJ0eXBlIjoiaGVsbG8ifQ").is_some());
        assert!(KeyEnvelope::parse("a-_Z09").is_some());
        assert!(KeyEnvelope::parse(&"a".repeat(MAX_KEY_ENVELOPE_BYTES)).is_some());
    }

    #[test]
    fn rejects_empty_oversized_and_foreign_characters() {
        for bad in ["", "abc=", "a b", "a+b", "a/b", "{\"x\":1}", "é"] {
            assert_eq!(KeyEnvelope::parse(bad), None, "{bad}");
        }
        assert_eq!(KeyEnvelope::parse(&"a".repeat(MAX_KEY_ENVELOPE_BYTES + 1)), None);
    }
}
