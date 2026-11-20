use std::fmt;

/// The identity provider's stable identifier for a user (the OIDC `sub`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct UserId(String);

impl UserId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// An authenticated user, as vouched for by the identity provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    pub id: UserId,
    pub username: String,
    pub email: Option<String>,
    roles: Vec<String>,
}

impl User {
    /// May administer Melior; implies everything a member may do.
    pub const ROLE_ADMIN: &'static str = "admin";
    /// May join rooms.
    pub const ROLE_MEMBER: &'static str = "member";

    pub fn new(
        id: UserId,
        username: impl Into<String>,
        email: Option<String>,
        roles: impl IntoIterator<Item = String>,
    ) -> Self {
        let mut roles: Vec<String> = roles.into_iter().collect();
        roles.sort();
        roles.dedup();
        Self { id, username: username.into(), email, roles }
    }

    /// The user's roles, sorted and without duplicates.
    #[must_use]
    pub fn roles(&self) -> &[String] {
        &self.roles
    }

    #[must_use]
    pub fn has_role(&self, role: &str) -> bool {
        self.roles.iter().any(|r| r == role)
    }

    /// Being authenticated is not enough: joining a room takes a Melior role.
    #[must_use]
    pub fn may_join_rooms(&self) -> bool {
        self.has_role(Self::ROLE_MEMBER) || self.has_role(Self::ROLE_ADMIN)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(roles: &[&str]) -> User {
        User::new(UserId::new("sub-1"), "alice", None, roles.iter().copied().map(str::to_owned))
    }

    #[test]
    fn roles_are_sorted_and_deduplicated() {
        assert_eq!(user(&["member", "admin", "member"]).roles(), ["admin", "member"]);
    }

    #[test]
    fn only_members_and_admins_may_join_rooms() {
        assert!(user(&["member"]).may_join_rooms());
        assert!(user(&["admin"]).may_join_rooms());
        assert!(!user(&[]).may_join_rooms());
        assert!(!user(&["viewer"]).may_join_rooms());
    }
}
