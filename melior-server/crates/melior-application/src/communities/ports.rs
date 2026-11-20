//! What communities need from the outside world: durable storage, an event bus
//! between instances, and the roster of who is in which voice channel.

use crate::conference::VoiceEntry;
use async_trait::async_trait;
use melior_domain::{
    Category, CategoryId, ChannelId, ChatMessage, Community, CommunityId, CommunityState, Invite,
    InviteCode, Member, Role, RoleId, SessionId, UserId,
};
use std::time::Duration;
use tokio::sync::broadcast;

#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
#[error("community store: {0}")]
pub struct CommunityStoreError(pub String);

/// What redeeming an invite did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Redemption {
    /// The user is a member now; the invite counted one use.
    Joined(CommunityId),
    /// The user already was a member; the invite was not used.
    AlreadyMember(CommunityId),
    /// The invite exists but has expired or is used up.
    NotUsable,
    Unknown,
}

/// Durable storage for communities, their structure, members and invites.
#[async_trait]
pub trait CommunityRepository: Send + Sync {
    /// Stores a new community with its structure and its first member.
    async fn create(
        &self,
        state: &CommunityState,
        owner: &Member,
    ) -> Result<(), CommunityStoreError>;

    async fn load(&self, id: &CommunityId) -> Result<Option<CommunityState>, CommunityStoreError>;

    /// The community a channel belongs to.
    async fn load_by_channel(
        &self,
        channel_id: &ChannelId,
    ) -> Result<Option<CommunityState>, CommunityStoreError>;

    /// The communities `user` is a member of, by name.
    async fn list_for(&self, user: &UserId) -> Result<Vec<Community>, CommunityStoreError>;

    async fn rename(&self, community: &Community) -> Result<(), CommunityStoreError>;

    /// Deletes a community with everything in it, its chat history included.
    async fn delete(&self, id: &CommunityId) -> Result<(), CommunityStoreError>;

    async fn member(
        &self,
        id: &CommunityId,
        user: &UserId,
    ) -> Result<Option<Member>, CommunityStoreError>;

    async fn members(&self, id: &CommunityId) -> Result<Vec<Member>, CommunityStoreError>;

    async fn member_count(&self, id: &CommunityId) -> Result<u64, CommunityStoreError>;

    async fn remove_member(
        &self,
        id: &CommunityId,
        user: &UserId,
    ) -> Result<(), CommunityStoreError>;

    /// Replaces a member's roles.
    async fn set_member_roles(
        &self,
        id: &CommunityId,
        member: &Member,
    ) -> Result<(), CommunityStoreError>;

    /// Inserts or replaces a role.
    async fn save_role(&self, id: &CommunityId, role: &Role) -> Result<(), CommunityStoreError>;

    /// Deletes a role, takes it from every member and drops the overrides naming it.
    async fn delete_role(&self, id: &CommunityId, role: &RoleId)
    -> Result<(), CommunityStoreError>;

    /// Inserts or replaces a category with its overrides.
    async fn save_category(
        &self,
        id: &CommunityId,
        category: &Category,
    ) -> Result<(), CommunityStoreError>;

    /// Deletes a category; its channels stay, without a category.
    async fn delete_category(
        &self,
        id: &CommunityId,
        category: &CategoryId,
    ) -> Result<(), CommunityStoreError>;

    /// Inserts or replaces a channel with its overrides.
    async fn save_channel(
        &self,
        id: &CommunityId,
        channel: &melior_domain::Channel,
    ) -> Result<(), CommunityStoreError>;

    /// Deletes a channel and its chat history.
    async fn delete_channel(&self, channel: &ChannelId) -> Result<(), CommunityStoreError>;

    async fn save_invite(&self, invite: &Invite) -> Result<(), CommunityStoreError>;

    async fn invite(&self, code: &InviteCode) -> Result<Option<Invite>, CommunityStoreError>;

    async fn invites(&self, id: &CommunityId) -> Result<Vec<Invite>, CommunityStoreError>;

    async fn delete_invite(&self, code: &InviteCode) -> Result<(), CommunityStoreError>;

    /// Makes `member` a member through an invite, atomically: two people redeeming
    /// the last use at once cannot both get in.
    async fn redeem_invite(
        &self,
        code: &InviteCode,
        member: &Member,
        now_ms: i64,
    ) -> Result<Redemption, CommunityStoreError>;
}

/// Why someone is no longer a member.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemovalReason {
    Kicked,
    Left,
}

/// Something every open feed of a community should hear about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommunityEvent {
    VoiceJoined(VoiceEntry),
    VoiceUpdated(VoiceEntry),
    VoiceLeft {
        channel_id: ChannelId,
        session_id: SessionId,
    },
    /// A message in a text channel.
    Message {
        channel_id: ChannelId,
        message: ChatMessage,
    },
    /// Channels, categories, roles, member roles, overrides or the name changed.
    StructureChanged,
    MemberRemoved {
        user_id: UserId,
        reason: RemovalReason,
    },
    CommunityDeleted,
}

#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
#[error("community events: {0}")]
pub struct EventsError(pub String);

/// Carries community events to every instance's subscribers.
#[async_trait]
pub trait CommunityEvents: Send + Sync {
    async fn publish(&self, id: &CommunityId, event: CommunityEvent) -> Result<(), EventsError>;

    /// The events of one community from now on. A receiver that falls behind
    /// misses events (and learns so from the channel); it never slows others down.
    fn subscribe(&self, id: &CommunityId) -> broadcast::Receiver<CommunityEvent>;
}

/// Who is in which voice channel of a community, across instances. Entries expire
/// unless they are put again, so a crashed instance's participants disappear.
#[async_trait]
pub trait VoiceRoster: Send + Sync {
    /// Adds or refreshes an entry, living `ttl` from now.
    async fn put(
        &self,
        id: &CommunityId,
        entry: &VoiceEntry,
        ttl: Duration,
    ) -> Result<(), CommunityStoreError>;

    async fn remove(
        &self,
        id: &CommunityId,
        session: &SessionId,
    ) -> Result<(), CommunityStoreError>;

    /// Every live entry of a community.
    async fn list(&self, id: &CommunityId) -> Result<Vec<VoiceEntry>, CommunityStoreError>;
}
