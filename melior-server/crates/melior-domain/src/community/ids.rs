//! The ids of a community and its parts, and invite codes.

use rand::Rng;
use rand::distributions::Alphanumeric;
use std::fmt;
use uuid::Uuid;

/// An id that is a lowercase hyphenated UUID. [`parse`](CommunityId::parse) accepts
/// exactly what `generate` produces, so a client cannot name one in another spelling.
macro_rules! uuid_id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(String);

        impl $name {
            #[must_use]
            pub fn generate() -> Self {
                Self(Uuid::new_v4().to_string())
            }

            #[must_use]
            pub fn parse(raw: &str) -> Option<Self> {
                let canonical = Uuid::parse_str(raw).ok()?.to_string();
                (canonical == raw).then_some(Self(canonical))
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

uuid_id!(
    /// Identifies a community.
    CommunityId
);
uuid_id!(
    /// Identifies a role within a community. The `@everyone` role's id is the
    /// community's id, as on Discord.
    RoleId
);
uuid_id!(
    /// Identifies a category of channels.
    CategoryId
);

impl RoleId {
    /// The id of a community's `@everyone` role.
    #[must_use]
    pub fn everyone(community: &CommunityId) -> Self {
        Self(community.as_str().to_owned())
    }
}

/// The code of an invite: 10 characters of `A-Z`, `a-z`, `0-9`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct InviteCode(String);

impl InviteCode {
    pub const LEN: usize = 10;

    #[must_use]
    pub fn generate() -> Self {
        let code =
            rand::thread_rng().sample_iter(&Alphanumeric).take(Self::LEN).map(char::from).collect();
        Self(code)
    }

    /// Accepts exactly the shape [`Self::generate`] produces.
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        let valid = raw.len() == Self::LEN && raw.bytes().all(|b| b.is_ascii_alphanumeric());
        valid.then(|| Self(raw.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for InviteCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_parse_what_generate_produces_and_nothing_else() {
        let id = CommunityId::generate();
        assert_eq!(CommunityId::parse(id.as_str()), Some(id.clone()));
        for bad in [
            String::new(),
            id.as_str().to_uppercase(),
            id.as_str().replace('-', ""),
            format!("{id} "),
            "general".to_owned(),
        ] {
            assert_eq!(CommunityId::parse(&bad), None, "{bad:?}");
            assert_eq!(RoleId::parse(&bad), None);
            assert_eq!(CategoryId::parse(&bad), None);
        }
        assert!(RoleId::parse(RoleId::generate().as_str()).is_some());
        assert!(CategoryId::parse(CategoryId::generate().as_str()).is_some());
    }

    #[test]
    fn the_everyone_role_shares_the_community_id() {
        let community = CommunityId::generate();
        assert_eq!(RoleId::everyone(&community).as_str(), community.as_str());
    }

    #[test]
    fn invite_codes_are_ten_alphanumeric_characters() {
        for _ in 0..50 {
            let code = InviteCode::generate();
            assert_eq!(InviteCode::parse(code.as_str()), Some(code));
        }
        assert_ne!(InviteCode::generate(), InviteCode::generate());
        assert!(InviteCode::parse("aB3dE5fG7h").is_some());
        for bad in ["", "aB3dE5fG7", "aB3dE5fG7hX", "aB3dE5fG7-", "aB3dE5fG7é", "aB3dE 5fG7"] {
            assert_eq!(InviteCode::parse(bad), None, "{bad:?}");
        }
    }
}
