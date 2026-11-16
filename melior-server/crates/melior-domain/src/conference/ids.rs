use super::DomainError;
use std::fmt;
use uuid::Uuid;

/// Identifies one participant's connection. Assigned by the server, never by the client.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SessionId(String);

impl SessionId {
    #[must_use]
    pub fn generate() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Identifies a track as it is forwarded to subscribers.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TrackId(String);

impl TrackId {
    #[must_use]
    pub fn generate() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TrackId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Names a room. Chosen by clients, so it is validated.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ChannelId(String);

impl ChannelId {
    pub const MAX_LEN: usize = 64;

    pub fn parse(value: &str) -> Result<Self, DomainError> {
        let valid_len = (1..=Self::MAX_LEN).contains(&value.len());
        let valid_chars =
            value.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
        if valid_len && valid_chars {
            Ok(Self(value.to_owned()))
        } else {
            Err(DomainError::InvalidChannelId)
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ChannelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_id_accepts_simple_names() {
        assert_eq!(ChannelId::parse("general").unwrap().as_str(), "general");
        assert!(ChannelId::parse("Team_7-standup").is_ok());
        assert!(ChannelId::parse(&"a".repeat(64)).is_ok());
    }

    #[test]
    fn channel_id_rejects_empty_long_and_unsafe_names() {
        for bad in ["", "has space", "slash/y", "ünï", &"a".repeat(65)] {
            assert_eq!(ChannelId::parse(bad), Err(DomainError::InvalidChannelId));
        }
    }

    #[test]
    fn generated_ids_are_unique() {
        assert_ne!(SessionId::generate(), SessionId::generate());
        assert_ne!(TrackId::generate(), TrackId::generate());
    }
}
