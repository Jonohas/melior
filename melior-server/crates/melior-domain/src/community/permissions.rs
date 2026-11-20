//! What a member may do, as a set of permission bits.

use super::CommunityError;
use std::fmt;

/// A set of permissions. Stored as its bits; sent over the wire as names.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Permissions(u64);

impl Permissions {
    pub const NONE: Self = Self(0);
    pub const VIEW_CHANNEL: Self = Self(1);
    pub const CONNECT: Self = Self(1 << 1);
    pub const SEND_MESSAGES: Self = Self(1 << 2);
    pub const READ_HISTORY: Self = Self(1 << 3);
    pub const CREATE_INVITE: Self = Self(1 << 4);
    pub const MANAGE_CHANNELS: Self = Self(1 << 5);
    pub const MANAGE_ROLES: Self = Self(1 << 6);
    pub const KICK_MEMBERS: Self = Self(1 << 7);
    pub const ADMINISTRATOR: Self = Self(1 << 8);
    pub const ALL: Self = Self((1 << 9) - 1);

    /// Every permission with its wire name, in bit order.
    pub const NAMED: [(Self, &'static str); 9] = [
        (Self::VIEW_CHANNEL, "view_channel"),
        (Self::CONNECT, "connect"),
        (Self::SEND_MESSAGES, "send_messages"),
        (Self::READ_HISTORY, "read_history"),
        (Self::CREATE_INVITE, "create_invite"),
        (Self::MANAGE_CHANNELS, "manage_channels"),
        (Self::MANAGE_ROLES, "manage_roles"),
        (Self::KICK_MEMBERS, "kick_members"),
        (Self::ADMINISTRATOR, "administrator"),
    ];

    /// What `@everyone` may do in a new community.
    pub const EVERYONE_DEFAULT: Self = Self(
        Self::VIEW_CHANNEL.0
            | Self::CONNECT.0
            | Self::SEND_MESSAGES.0
            | Self::READ_HISTORY.0
            | Self::CREATE_INVITE.0,
    );

    /// The stored form. Bits that name no permission are dropped.
    #[must_use]
    pub fn from_bits(bits: u64) -> Self {
        Self(bits & Self::ALL.0)
    }

    #[must_use]
    pub fn bits(self) -> u64 {
        self.0
    }

    #[must_use]
    pub fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// `self` without the permissions in `other`.
    #[must_use]
    pub fn difference(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }

    /// Whether every permission in `other` is in `self`.
    #[must_use]
    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    #[must_use]
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The wire names of the permissions in the set, in bit order.
    #[must_use]
    pub fn names(self) -> Vec<&'static str> {
        Self::NAMED.iter().filter(|(bit, _)| self.contains(*bit)).map(|(_, name)| *name).collect()
    }

    /// The wire name of a single permission; `"?"` for a set that is not one bit.
    #[must_use]
    pub fn name(self) -> &'static str {
        Self::NAMED.iter().find(|(bit, _)| *bit == self).map_or("?", |(_, name)| *name)
    }

    /// A set from wire names. An unknown name is refused, not ignored.
    pub fn from_names<S: AsRef<str>>(
        names: impl IntoIterator<Item = S>,
    ) -> Result<Self, CommunityError> {
        names.into_iter().try_fold(Self::NONE, |set, name| {
            let name = name.as_ref();
            Self::NAMED
                .iter()
                .find(|(_, wire)| *wire == name)
                .map(|(bit, _)| set.union(*bit))
                .ok_or_else(|| CommunityError::UnknownPermission(name.chars().take(64).collect()))
        })
    }
}

impl fmt::Debug for Permissions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Permissions").field(&self.names()).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_holds_every_named_permission_and_none_holds_nothing() {
        for (bit, _) in Permissions::NAMED {
            assert!(Permissions::ALL.contains(bit));
            assert!(!Permissions::NONE.contains(bit));
        }
        assert_eq!(Permissions::ALL.names().len(), 9);
        assert!(Permissions::NONE.is_empty());
    }

    #[test]
    fn union_and_difference_combine_sets() {
        let both = Permissions::VIEW_CHANNEL.union(Permissions::CONNECT);
        assert!(both.contains(Permissions::VIEW_CHANNEL));
        assert!(both.contains(Permissions::CONNECT));
        assert_eq!(both.difference(Permissions::CONNECT), Permissions::VIEW_CHANNEL);
        assert!(!Permissions::VIEW_CHANNEL.contains(both));
    }

    #[test]
    fn names_round_trip_in_bit_order() {
        let set = Permissions::from_names(["connect", "view_channel", "administrator"]).unwrap();
        assert_eq!(set.names(), ["view_channel", "connect", "administrator"]);
        assert_eq!(Permissions::from_names(set.names()).unwrap(), set);
        assert_eq!(Permissions::from_names(Vec::<String>::new()).unwrap(), Permissions::NONE);
        assert_eq!(Permissions::KICK_MEMBERS.name(), "kick_members");
    }

    #[test]
    fn an_unknown_name_is_refused() {
        assert_eq!(
            Permissions::from_names(["view_channel", "fly"]),
            Err(CommunityError::UnknownPermission("fly".into()))
        );
        assert!(Permissions::from_names(["VIEW_CHANNEL"]).is_err());
    }

    #[test]
    fn the_default_for_everyone_is_the_everyday_set() {
        assert_eq!(
            Permissions::EVERYONE_DEFAULT.names(),
            ["view_channel", "connect", "send_messages", "read_history", "create_invite"]
        );
    }

    #[test]
    fn stored_bits_round_trip_and_unknown_bits_are_dropped() {
        let set = Permissions::from_names(["manage_roles", "kick_members"]).unwrap();
        assert_eq!(Permissions::from_bits(set.bits()), set);
        assert_eq!(Permissions::from_bits(u64::MAX), Permissions::ALL);
    }
}
