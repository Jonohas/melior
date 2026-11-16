use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::RngCore;
use std::fmt;

/// Identifies a login session; the value of the session cookie.
///
/// 32 random bytes, base64url without padding. Knowing it is being logged in, so it
/// comes from the operating system's random source and is never derived from anything.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct AuthSessionId(String);

impl AuthSessionId {
    const BYTES: usize = 32;

    #[must_use]
    pub fn generate() -> Self {
        let mut bytes = [0u8; Self::BYTES];
        rand::rngs::OsRng.fill_bytes(&mut bytes);
        Self(URL_SAFE_NO_PAD.encode(bytes))
    }

    /// Accepts only the canonical form [`Self::generate`] produces, so arbitrary cookie
    /// values never reach the session store.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        let bytes = URL_SAFE_NO_PAD.decode(value).ok()?;
        (bytes.len() == Self::BYTES && URL_SAFE_NO_PAD.encode(&bytes) == value)
            .then(|| Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Never printed in full: it is a credential.
impl fmt::Debug for AuthSessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AuthSessionId({}…)", &self.0[..6])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_ids_round_trip_and_differ() {
        let id = AuthSessionId::generate();
        assert_eq!(id.as_str().len(), 43);
        assert_eq!(AuthSessionId::parse(id.as_str()), Some(id.clone()));
        assert_ne!(id, AuthSessionId::generate());
    }

    #[test]
    fn rejects_anything_not_canonical() {
        let id = AuthSessionId::generate();
        for bad in [
            "",
            "short",
            &format!("{}=", id.as_str()),
            &format!("{}A", id.as_str()),
            &id.as_str().replace(|c: char| c.is_ascii_alphanumeric(), "!"),
            "../../etc/passwd",
        ] {
            assert_eq!(AuthSessionId::parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn debug_output_does_not_reveal_the_id() {
        let id = AuthSessionId::generate();
        assert!(!format!("{id:?}").contains(id.as_str()));
    }
}
