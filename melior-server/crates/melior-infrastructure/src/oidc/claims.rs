//! The token claims this application reads, and how they become a [`User`].

use melior_domain::{User, UserId};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub(super) struct IdTokenClaims {
    pub sub: String,
    pub nonce: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct AccessTokenClaims {
    pub sub: String,
    pub exp: u64,
    pub preferred_username: Option<String>,
    pub email: Option<String>,
    #[serde(default)]
    pub resource_access: HashMap<String, ClientAccess>,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct ClientAccess {
    #[serde(default)]
    pub roles: Vec<String>,
}

impl AccessTokenClaims {
    /// The user this token describes. Roles are the client roles granted on
    /// `client_id` (`resource_access.<client_id>.roles`); realm roles and roles on
    /// other clients are deliberately ignored.
    pub fn into_user(mut self, client_id: &str) -> User {
        let roles = self.resource_access.remove(client_id).unwrap_or_default().roles;
        let username = self
            .preferred_username
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| self.sub.clone());
        User::new(UserId::new(self.sub), username, self.email, roles)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_identity_and_this_clients_roles_only() {
        let claims: AccessTokenClaims = serde_json::from_value(json!({
            "sub": "f3c1",
            "exp": 1_800_000_000u64,
            "preferred_username": "alice",
            "email": "alice@example.test",
            "realm_access": { "roles": ["offline_access", "admin"] },
            "resource_access": {
                "melior": { "roles": ["member"] },
                "account": { "roles": ["manage-account"] }
            }
        }))
        .unwrap();

        let user = claims.into_user("melior");
        assert_eq!(user.id.as_str(), "f3c1");
        assert_eq!(user.username, "alice");
        assert_eq!(user.email.as_deref(), Some("alice@example.test"));
        assert_eq!(user.roles(), ["member"]);
    }

    #[test]
    fn tolerates_missing_optional_claims() {
        let claims: AccessTokenClaims =
            serde_json::from_value(json!({ "sub": "f3c1", "exp": 1u64 })).unwrap();

        let user = claims.into_user("melior");
        assert_eq!(user.username, "f3c1", "falls back to the subject");
        assert_eq!(user.email, None);
        assert!(user.roles().is_empty());
        assert!(!user.may_join_rooms());
    }
}
