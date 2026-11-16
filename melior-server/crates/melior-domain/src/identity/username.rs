use crate::DomainError;
use std::fmt;

/// A login name in the form people are invited by: lower-case, 1 to 64 characters
/// of `a-z`, `0-9`, `.`, `_`, `@` and `-`.
///
/// Names from the identity provider and names typed into an invitation list go
/// through the same normalisation, so they compare equal whatever their case.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Username(String);

impl Username {
    pub const MAX_LEN: usize = 64;

    pub fn parse(value: &str) -> Result<Self, DomainError> {
        let normalised = value.trim().to_lowercase();
        let valid_len = (1..=Self::MAX_LEN).contains(&normalised.len());
        let valid_chars = normalised
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b".-_@".contains(&b));
        if valid_len && valid_chars {
            Ok(Self(normalised))
        } else {
            Err(DomainError::InvalidUsername(value.chars().take(Self::MAX_LEN).collect()))
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Username {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalises_case_and_surrounding_space() {
        assert_eq!(Username::parse("  Alice ").unwrap().as_str(), "alice");
        assert_eq!(
            Username::parse("bob.smith@example-1.test").unwrap().as_str(),
            "bob.smith@example-1.test"
        );
        assert_eq!(Username::parse("ALICE").unwrap(), Username::parse("alice").unwrap());
    }

    #[test]
    fn rejects_empty_long_and_unusual_names() {
        for bad in ["", "   ", "has space", "semi;colon", "ünï", "a/b", &"a".repeat(65)] {
            assert!(
                matches!(Username::parse(bad), Err(DomainError::InvalidUsername(_))),
                "{bad:?}"
            );
        }
    }
}
