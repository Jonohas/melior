use super::ports::{RepositoryError, RoomRepository};
use melior_domain::{ChannelId, DomainError, RoomAccess, User, Username};
use std::sync::Arc;
use tracing::info;

#[derive(thiserror::Error, Debug)]
pub enum RoomsError {
    #[error("room is not registered")]
    NotFound,

    #[error("not allowed")]
    Forbidden,

    #[error(transparent)]
    Domain(#[from] DomainError),

    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

/// The room registry use cases.
pub struct RoomAccessService {
    rooms: Arc<dyn RoomRepository>,
}

impl RoomAccessService {
    pub fn new(rooms: Arc<dyn RoomRepository>) -> Self {
        Self { rooms }
    }

    /// A room's access settings, for someone who may enter it.
    pub async fn get(&self, channel_id: &ChannelId, user: &User) -> Result<RoomAccess, RoomsError> {
        let room = self.rooms.find(channel_id).await?.ok_or(RoomsError::NotFound)?;
        if room.may_join(user) { Ok(room) } else { Err(RoomsError::Forbidden) }
    }

    /// The rooms a user owns or is invited to; every room for an admin.
    pub async fn list(&self, user: &User) -> Result<Vec<RoomAccess>, RoomsError> {
        if !user.may_join_rooms() {
            return Err(RoomsError::Forbidden);
        }
        let rooms = if user.has_role(User::ROLE_ADMIN) {
            self.rooms.list_all().await?
        } else {
            self.rooms.list_for(user).await?
        };
        Ok(rooms)
    }

    /// Sets who may enter a room. Registers the room, owned by `user`, if it does
    /// not exist yet; otherwise only its owner or an admin may change it.
    #[tracing::instrument(name = "rooms.configure", skip_all, fields(channel = %channel_id, user = %user.username))]
    pub async fn configure(
        &self,
        channel_id: &ChannelId,
        user: &User,
        restricted: bool,
        members: &[String],
    ) -> Result<RoomAccess, RoomsError> {
        if !user.may_join_rooms() {
            return Err(RoomsError::Forbidden);
        }
        let members =
            members.iter().map(|name| Username::parse(name)).collect::<Result<Vec<_>, _>>()?;

        let mut room =
            self.rooms.insert_if_absent(&RoomAccess::open(channel_id.clone(), user)).await?;
        if !room.may_manage(user) {
            return Err(RoomsError::Forbidden);
        }
        room.configure(restricted, members);
        self.rooms.save(&room).await?;
        info!(channel = %channel_id, user = %user.username, restricted, "room access changed");
        Ok(room)
    }

    /// Removes a room from the registry. The channel is then unregistered again:
    /// the next person to join it becomes its owner.
    pub async fn delete(&self, channel_id: &ChannelId, user: &User) -> Result<(), RoomsError> {
        let room = self.rooms.find(channel_id).await?.ok_or(RoomsError::NotFound)?;
        if !room.may_manage(user) {
            return Err(RoomsError::Forbidden);
        }
        self.rooms.delete(channel_id).await?;
        info!(channel = %channel_id, user = %user.username, "room deleted");
        Ok(())
    }

    /// Decides whether `user` may enter a room right now. A channel nobody has
    /// registered becomes an open room owned by `user`.
    #[tracing::instrument(name = "rooms.admit", skip_all, fields(channel = %channel_id, user = %user.username))]
    pub async fn admit(&self, channel_id: &ChannelId, user: &User) -> Result<(), RoomsError> {
        if !user.may_join_rooms() {
            return Err(RoomsError::Forbidden);
        }
        let room = self.rooms.insert_if_absent(&RoomAccess::open(channel_id.clone(), user)).await?;
        if room.may_join(user) { Ok(()) } else { Err(RoomsError::Forbidden) }
    }

    /// Whether a user who is already in a room may stay. Unlike [`Self::admit`] this
    /// registers nothing: a room deleted meanwhile is simply unregistered, so open.
    pub async fn may_stay(&self, channel_id: &ChannelId, user: &User) -> Result<bool, RoomsError> {
        if !user.may_join_rooms() {
            return Ok(false);
        }
        Ok(self.rooms.find(channel_id).await?.is_none_or(|room| room.may_join(user)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rooms::InMemoryRoomRepository;
    use melior_domain::UserId;

    fn user(name: &str, roles: &[&str]) -> User {
        User::new(
            UserId::new(format!("sub-{name}")),
            name,
            None,
            roles.iter().copied().map(str::to_owned),
        )
    }

    fn channel(name: &str) -> ChannelId {
        ChannelId::parse(name).unwrap()
    }

    fn service() -> RoomAccessService {
        RoomAccessService::new(Arc::new(InMemoryRoomRepository::default()))
    }

    fn names(names: &[&str]) -> Vec<String> {
        names.iter().map(|n| (*n).to_owned()).collect()
    }

    #[tokio::test]
    async fn the_first_to_join_owns_an_open_room() {
        let s = service();
        let (alice, bob) = (user("alice", &["member"]), user("bob", &["member"]));

        s.admit(&channel("standup"), &alice).await.unwrap();
        s.admit(&channel("standup"), &bob).await.unwrap();

        let room = s.get(&channel("standup"), &bob).await.unwrap();
        assert_eq!(room.owner_username(), "alice");
        assert!(!room.is_restricted());
        assert!(matches!(s.get(&channel("other"), &bob).await, Err(RoomsError::NotFound)));
    }

    #[tokio::test]
    async fn a_restricted_room_refuses_the_uninvited_at_the_door_and_inside() {
        let s = service();
        let (alice, bob, carol) =
            (user("alice", &["member"]), user("bob", &["member"]), user("carol", &["member"]));
        s.admit(&channel("board"), &alice).await.unwrap();
        s.admit(&channel("board"), &carol).await.unwrap();

        let room = s.configure(&channel("board"), &alice, true, &names(&["Bob"])).await.unwrap();
        assert!(room.is_restricted());

        s.admit(&channel("board"), &bob).await.unwrap();
        assert!(matches!(s.admit(&channel("board"), &carol).await, Err(RoomsError::Forbidden)));
        assert!(matches!(s.get(&channel("board"), &carol).await, Err(RoomsError::Forbidden)));
        assert!(
            !s.may_stay(&channel("board"), &carol).await.unwrap(),
            "carol was inside and must go"
        );
        assert!(s.may_stay(&channel("board"), &bob).await.unwrap());
        assert!(s.may_stay(&channel("board"), &alice).await.unwrap());
    }

    #[tokio::test]
    async fn only_the_owner_or_an_admin_changes_or_deletes_a_room() {
        let s = service();
        let (alice, bob, root) =
            (user("alice", &["member"]), user("bob", &["member"]), user("root", &["admin"]));
        s.admit(&channel("board"), &alice).await.unwrap();

        assert!(matches!(
            s.configure(&channel("board"), &bob, true, &[]).await,
            Err(RoomsError::Forbidden)
        ));
        assert!(matches!(s.delete(&channel("board"), &bob).await, Err(RoomsError::Forbidden)));

        s.configure(&channel("board"), &root, true, &names(&["bob"])).await.unwrap();
        s.delete(&channel("board"), &alice).await.unwrap();
        assert!(matches!(s.delete(&channel("board"), &alice).await, Err(RoomsError::NotFound)));
        assert!(s.may_stay(&channel("board"), &bob).await.unwrap(), "unregistered means open");
    }

    #[tokio::test]
    async fn configuring_an_unregistered_room_registers_it_for_the_caller() {
        let s = service();
        let bob = user("bob", &["member"]);

        let room = s.configure(&channel("private"), &bob, true, &[]).await.unwrap();

        assert_eq!(room.owner_username(), "bob");
        assert!(matches!(
            s.admit(&channel("private"), &user("alice", &["member"])).await,
            Err(RoomsError::Forbidden)
        ));
    }

    #[tokio::test]
    async fn invalid_usernames_change_nothing() {
        let s = service();
        let alice = user("alice", &["member"]);
        s.admit(&channel("board"), &alice).await.unwrap();

        let result =
            s.configure(&channel("board"), &alice, true, &names(&["bob", "not valid"])).await;

        assert!(matches!(result, Err(RoomsError::Domain(DomainError::InvalidUsername(_)))));
        assert!(!s.get(&channel("board"), &alice).await.unwrap().is_restricted());
    }

    #[tokio::test]
    async fn listing_shows_owned_and_invited_rooms_and_everything_to_admins() {
        let s = service();
        let (alice, bob, root) =
            (user("alice", &["member"]), user("bob", &["member"]), user("root", &["admin"]));
        s.configure(&channel("a-room"), &alice, true, &names(&["bob"])).await.unwrap();
        s.configure(&channel("b-room"), &bob, false, &[]).await.unwrap();
        s.configure(&channel("c-room"), &alice, true, &[]).await.unwrap();

        let ids = |rooms: Vec<RoomAccess>| -> Vec<String> {
            rooms.iter().map(|room| room.channel_id().to_string()).collect()
        };
        assert_eq!(ids(s.list(&alice).await.unwrap()), ["a-room", "c-room"]);
        assert_eq!(ids(s.list(&bob).await.unwrap()), ["a-room", "b-room"]);
        assert_eq!(ids(s.list(&root).await.unwrap()), ["a-room", "b-room", "c-room"]);
        assert!(matches!(s.list(&user("guest", &[])).await, Err(RoomsError::Forbidden)));
    }

    #[tokio::test]
    async fn users_without_a_role_are_refused_everywhere() {
        let s = service();
        let guest = user("guest", &[]);

        assert!(matches!(s.admit(&channel("x"), &guest).await, Err(RoomsError::Forbidden)));
        assert!(matches!(
            s.configure(&channel("x"), &guest, false, &[]).await,
            Err(RoomsError::Forbidden)
        ));
        assert!(!s.may_stay(&channel("x"), &guest).await.unwrap());
    }
}
