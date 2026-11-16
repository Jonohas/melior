use super::ChannelId;
use crate::identity::{User, UserId, Username};
use std::collections::BTreeSet;

/// Who may enter a room: the room's entry in the registry.
///
/// A room is registered the first time someone joins it, open, with that person as
/// owner. The owner (or an admin) can restrict it to invited usernames.
///
/// Invariant: the owner and admins can always join and manage a room, whatever the
/// invitation list says, so a room can never lock everyone out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoomAccess {
    channel_id: ChannelId,
    owner_id: UserId,
    owner_username: String,
    restricted: bool,
    members: BTreeSet<Username>,
}

impl RoomAccess {
    /// A new room anyone with a Melior role may join, owned by `owner`.
    #[must_use]
    pub fn open(channel_id: ChannelId, owner: &User) -> Self {
        Self {
            channel_id,
            owner_id: owner.id.clone(),
            owner_username: owner.username.clone(),
            restricted: false,
            members: BTreeSet::new(),
        }
    }

    /// Rebuilds a room from storage.
    #[must_use]
    pub fn restore(
        channel_id: ChannelId,
        owner_id: UserId,
        owner_username: String,
        restricted: bool,
        members: impl IntoIterator<Item = Username>,
    ) -> Self {
        Self {
            channel_id,
            owner_id,
            owner_username,
            restricted,
            members: members.into_iter().collect(),
        }
    }

    #[must_use]
    pub fn channel_id(&self) -> &ChannelId {
        &self.channel_id
    }

    #[must_use]
    pub fn owner_id(&self) -> &UserId {
        &self.owner_id
    }

    #[must_use]
    pub fn owner_username(&self) -> &str {
        &self.owner_username
    }

    #[must_use]
    pub fn is_restricted(&self) -> bool {
        self.restricted
    }

    /// The invited usernames, sorted.
    pub fn members(&self) -> impl Iterator<Item = &Username> {
        self.members.iter()
    }

    /// The owner and admins manage a room.
    #[must_use]
    pub fn may_manage(&self, user: &User) -> bool {
        user.id == self.owner_id || user.has_role(User::ROLE_ADMIN)
    }

    /// Whether `user` may enter. Needs a Melior role in every case; a restricted
    /// room additionally needs an invitation, unless the user manages the room.
    #[must_use]
    pub fn may_join(&self, user: &User) -> bool {
        if !user.may_join_rooms() {
            return false;
        }
        !self.restricted || self.may_manage(user) || self.is_invited(user)
    }

    /// True if the user is on the invitation list (whether or not the room is restricted).
    #[must_use]
    pub fn is_invited(&self, user: &User) -> bool {
        Username::parse(&user.username).is_ok_and(|name| self.members.contains(&name))
    }

    /// Replaces the access settings.
    pub fn configure(&mut self, restricted: bool, members: impl IntoIterator<Item = Username>) {
        self.restricted = restricted;
        self.members = members.into_iter().collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(name: &str, roles: &[&str]) -> User {
        User::new(
            UserId::new(format!("sub-{name}")),
            name,
            None,
            roles.iter().copied().map(str::to_owned),
        )
    }

    fn room(owner: &User) -> RoomAccess {
        RoomAccess::open(ChannelId::parse("standup").unwrap(), owner)
    }

    fn names(names: &[&str]) -> Vec<Username> {
        names.iter().map(|n| Username::parse(n).unwrap()).collect()
    }

    #[test]
    fn a_new_room_is_open_to_every_member_but_not_to_users_without_a_role() {
        let alice = user("alice", &["member"]);
        let room = room(&alice);

        assert!(!room.is_restricted());
        assert!(room.may_join(&alice));
        assert!(room.may_join(&user("bob", &["member"])));
        assert!(!room.may_join(&user("guest", &[])));
    }

    #[test]
    fn a_restricted_room_admits_owner_invited_and_admins_only() {
        let alice = user("alice", &["member"]);
        let mut room = room(&alice);
        room.configure(true, names(&["Bob"]));

        assert!(room.may_join(&alice), "the owner is never locked out");
        assert!(room.may_join(&user("bob", &["member"])));
        assert!(room.may_join(&user("BOB", &["member"])), "names compare case-insensitively");
        assert!(!room.may_join(&user("carol", &["member"])));
        assert!(room.may_join(&user("root", &["admin"])));
        assert!(!room.may_join(&user("bob", &[])), "an invitation does not replace a role");
        assert_eq!(room.members().map(Username::as_str).collect::<Vec<_>>(), ["bob"]);
    }

    #[test]
    fn only_the_owner_and_admins_manage_a_room() {
        let alice = user("alice", &["member"]);
        let mut room = room(&alice);
        room.configure(true, names(&["bob"]));

        assert!(room.may_manage(&alice));
        assert!(room.may_manage(&user("root", &["admin"])));
        assert!(!room.may_manage(&user("bob", &["member"])));
        // Same name, different account: ownership follows the user id.
        let impostor = User::new(UserId::new("sub-other"), "alice", None, ["member".to_owned()]);
        assert!(!room.may_manage(&impostor));
    }

    #[test]
    fn reopening_a_room_lets_everyone_back_in() {
        let alice = user("alice", &["member"]);
        let mut room = room(&alice);
        room.configure(true, names(&[]));
        assert!(!room.may_join(&user("carol", &["member"])));

        room.configure(false, names(&[]));
        assert!(room.may_join(&user("carol", &["member"])));
    }
}
