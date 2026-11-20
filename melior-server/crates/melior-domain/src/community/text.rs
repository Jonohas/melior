//! Names and topics, checked the way chat text is: trimmed, counted in characters,
//! without control characters.

use super::CommunityError;
use std::fmt;

/// The name of a community, role, category or channel: trimmed, 1 to 100
/// characters, no control characters.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Name(pub(super) String);

impl Name {
    pub const MAX_CHARS: usize = 100;

    pub fn parse(raw: &str) -> Result<Self, CommunityError> {
        let name = raw.trim();
        let valid = (1..=Self::MAX_CHARS).contains(&name.chars().count())
            && !name.chars().any(char::is_control);
        if !valid {
            return Err(CommunityError::InvalidName);
        }
        Ok(Self(name.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A channel's topic: trimmed, at most 1024 characters; line breaks and tabs are
/// the only control characters allowed.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Topic(String);

impl Topic {
    pub const MAX_CHARS: usize = 1024;

    pub fn parse(raw: &str) -> Result<Self, CommunityError> {
        let topic = raw.trim();
        let valid = topic.chars().count() <= Self::MAX_CHARS
            && !topic.chars().any(|c| c.is_control() && !matches!(c, '\n' | '\t'));
        if !valid {
            return Err(CommunityError::InvalidTopic);
        }
        Ok(Self(topic.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_is_trimmed_and_one_to_a_hundred_characters() {
        assert_eq!(Name::parse("  General ").unwrap().as_str(), "General");
        assert!(Name::parse(&"é".repeat(100)).is_ok(), "characters, not bytes");
        for bad in
            [String::new(), "   ".into(), "x".repeat(101), "tab\there".into(), "nul\0".into()]
        {
            assert_eq!(Name::parse(&bad), Err(CommunityError::InvalidName), "{bad:?}");
        }
    }

    #[test]
    fn a_topic_may_be_empty_and_hold_line_breaks() {
        assert_eq!(Topic::parse("  ").unwrap().as_str(), "");
        assert!(Topic::parse("one\ntwo").is_ok());
        assert!(Topic::parse(&"x".repeat(1024)).is_ok());
        assert_eq!(Topic::parse(&"x".repeat(1025)), Err(CommunityError::InvalidTopic));
        assert_eq!(Topic::parse("bell\u{7}"), Err(CommunityError::InvalidTopic));
    }
}
