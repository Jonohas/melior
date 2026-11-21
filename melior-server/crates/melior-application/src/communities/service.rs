//! The community use cases. Each loads what it needs, asks the domain, persists
//! the result and tells the community's open feeds.

use super::ports::{
    CommunityEvent, CommunityEvents, CommunityRepository, CommunityStoreError, VoiceRoster,
};
use melior_domain::{
    Category, Channel, ChannelId, Community, CommunityError, CommunityId, CommunityState, Member,
    MeshPolicy, Name, Permissions, Rank, Refusal, Rejected, Role, RoomPlan, Standing, User,
};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{info, warn};

mod access;
mod invites;
mod roles;
mod structure;

pub use access::{ChannelGrant, VoiceGrant};
pub use invites::InvitePreview;

#[derive(thiserror::Error, Debug)]
pub enum CommunitiesError {
    /// No such community, channel, role, category or invite.
    #[error("not found")]
    NotFound,

    /// Not a member, or no Melior role.
    #[error("not allowed")]
    Forbidden,

    /// A member who may not do this; the wording is fit for them.
    #[error(transparent)]
    Refused(#[from] Refusal),

    #[error(transparent)]
    Invalid(#[from] CommunityError),

    /// The channel is of the other kind.
    #[error("not possible in this kind of channel")]
    WrongKind,

    /// The voice channel holds as many people as it admits.
    #[error("this channel is full")]
    Full,

    /// The invite has expired or is used up.
    #[error("this invite is no longer valid")]
    Gone,

    #[error(transparent)]
    Store(#[from] CommunityStoreError),
}

impl From<Rejected> for CommunitiesError {
    fn from(rejected: Rejected) -> Self {
        match rejected {
            Rejected::Invalid(error) => Self::Invalid(error),
            Rejected::Refused(refusal) => Self::Refused(refusal),
        }
    }
}

/// A community as one member sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommunityView {
    pub community: Community,
    pub me: MemberView,
    pub roles: Vec<Role>,
    pub categories: Vec<Category>,
    /// Only the channels the member may view, by position.
    pub channels: Vec<ChannelView>,
}

/// The viewing member: who they are and what they may do community-wide.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberView {
    pub member: Member,
    pub permissions: Permissions,
    pub rank: Rank,
    pub is_owner: bool,
}

/// A channel as one member sees it, with how its room runs on this instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelView {
    pub community_id: CommunityId,
    pub channel: Channel,
    pub plan: RoomPlan,
    pub permissions: Permissions,
}

/// A member in their community: the loaded state, the member and their standing.
pub(super) struct Seat {
    pub(super) state: CommunityState,
    pub(super) member: Member,
    pub(super) standing: Standing,
}

/// The community use cases.
pub struct CommunityService {
    pub(super) store: Arc<dyn CommunityRepository>,
    pub(super) events: Arc<dyn CommunityEvents>,
    pub(super) roster: Arc<dyn VoiceRoster>,
    pub(super) mesh: MeshPolicy,
}

impl CommunityService {
    pub fn new(
        store: Arc<dyn CommunityRepository>,
        events: Arc<dyn CommunityEvents>,
        roster: Arc<dyn VoiceRoster>,
        mesh: MeshPolicy,
    ) -> Self {
        Self { store, events, roster, mesh }
    }

    /// The event bus, for the feeds that subscribe to it.
    #[must_use]
    pub fn events(&self) -> &Arc<dyn CommunityEvents> {
        &self.events
    }

    /// The communities the user is a member of.
    pub async fn list(&self, user: &User) -> Result<Vec<Community>, CommunitiesError> {
        require_role(user)?;
        Ok(self.store.list_for(&user.id).await?)
    }

    /// A new community owned by the user, seeded with its first channels.
    #[tracing::instrument(name = "communities.create", skip_all, fields(user = %user.username))]
    pub async fn create(&self, user: &User, name: &str) -> Result<CommunityView, CommunitiesError> {
        require_role(user)?;
        let name = Name::parse(name)?;
        let (state, owner) = CommunityState::create(CommunityId::generate(), name, user);
        self.store.create(&state, &owner).await?;
        info!(community = %state.id(), user = %user.username, "community created");
        Ok(self.view_of(state, owner))
    }

    /// The community as the user sees it.
    pub async fn view(
        &self,
        id: &CommunityId,
        user: &User,
    ) -> Result<CommunityView, CommunitiesError> {
        let seat = self.seat(id, user).await?;
        Ok(self.view_of(seat.state, seat.member))
    }

    /// Renames the community: the owner or an administrator.
    pub async fn rename(
        &self,
        id: &CommunityId,
        user: &User,
        name: &str,
    ) -> Result<CommunityView, CommunitiesError> {
        let name = Name::parse(name)?;
        let mut seat = self.seat(id, user).await?;
        let community = seat.state.rename(&seat.standing, name)?;
        self.store.rename(&community).await?;
        seat.state.community = community;
        self.publish(id, CommunityEvent::StructureChanged).await;
        Ok(self.view_of(seat.state, seat.member))
    }

    /// Deletes the community with its channels and their history: the owner only.
    #[tracing::instrument(name = "communities.delete", skip_all, fields(community = %id))]
    pub async fn delete(&self, id: &CommunityId, user: &User) -> Result<(), CommunitiesError> {
        let seat = self.seat(id, user).await?;
        seat.standing.may_delete_community()?;
        self.store.delete(id).await?;
        info!(community = %id, user = %user.username, "community deleted");
        self.publish(id, CommunityEvent::CommunityDeleted).await;
        Ok(())
    }

    /// Builds what `member` sees of `state`.
    pub(super) fn view_of(&self, state: CommunityState, member: Member) -> CommunityView {
        let standing = state.standing(&member);
        let channels = state
            .visible_channels(&standing)
            .into_iter()
            .map(|(channel, permissions)| self.channel_view(state.id(), channel, permissions))
            .collect();
        CommunityView {
            me: MemberView {
                permissions: standing.base(),
                rank: standing.rank(),
                is_owner: standing.is_owner(),
                member,
            },
            channels,
            roles: sorted_roles(state.roles),
            categories: sorted_categories(state.categories),
            community: state.community,
        }
    }

    pub(super) fn channel_view(
        &self,
        community_id: &CommunityId,
        channel: &Channel,
        permissions: Permissions,
    ) -> ChannelView {
        ChannelView {
            community_id: community_id.clone(),
            plan: channel.plan(self.mesh),
            channel: channel.clone(),
            permissions,
        }
    }

    /// Loads a community and the user's membership of it.
    pub(super) async fn seat(
        &self,
        id: &CommunityId,
        user: &User,
    ) -> Result<Seat, CommunitiesError> {
        require_role(user)?;
        let state = self.store.load(id).await?.ok_or(CommunitiesError::NotFound)?;
        self.seat_in(state, user).await
    }

    /// Loads the community of a channel and the user's membership of it.
    pub(super) async fn seat_by_channel(
        &self,
        channel_id: &ChannelId,
        user: &User,
    ) -> Result<Seat, CommunitiesError> {
        require_role(user)?;
        let state =
            self.store.load_by_channel(channel_id).await?.ok_or(CommunitiesError::NotFound)?;
        self.seat_in(state, user).await
    }

    async fn seat_in(&self, state: CommunityState, user: &User) -> Result<Seat, CommunitiesError> {
        let member =
            self.store.member(state.id(), &user.id).await?.ok_or(CommunitiesError::Forbidden)?;
        let standing = state.standing(&member);
        Ok(Seat { state, member, standing })
    }

    /// Tells the community's feeds. A failing bus is logged: the change is made,
    /// and feeds catch up at their next refresh.
    pub(super) async fn publish(&self, id: &CommunityId, event: CommunityEvent) {
        if let Err(error) = self.events.publish(id, event).await {
            warn!(%error, community = %id, "community event not published");
        }
    }
}

/// Nothing in a community is open to a user without a Melior role.
pub(super) fn require_role(user: &User) -> Result<(), CommunitiesError> {
    if user.may_join_rooms() {
        return Ok(());
    }
    Err(CommunitiesError::Forbidden)
}

pub(super) fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX))
}

fn sorted_roles(mut roles: Vec<Role>) -> Vec<Role> {
    roles.sort_by(|a, b| (a.position, &a.id).cmp(&(b.position, &b.id)));
    roles
}

fn sorted_categories(mut categories: Vec<Category>) -> Vec<Category> {
    categories.sort_by(|a, b| (a.position, &a.id).cmp(&(b.position, &b.id)));
    categories
}
