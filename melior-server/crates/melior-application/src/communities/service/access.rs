//! Who may enter which channel and what they may do there: voice admission, the
//! re-check of open connections, text channel posting and history, the feed.

use super::{CommunitiesError, CommunityService, Seat};
use crate::communities::feed::FeedFilter;
use crate::conference::VoiceEntry;
use melior_domain::{
    ChannelId, ChannelKind, ChannelMedia, CommunityId, MeshPolicy, Permissions, Refusal, RoomPlan,
    User,
};

/// What a member was admitted to a voice channel with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceGrant {
    pub community_id: CommunityId,
    pub channel_media: ChannelMedia,
    /// What the member may do in the channel.
    pub permissions: Permissions,
}

impl VoiceGrant {
    /// How the channel's room runs on an instance with `mesh`.
    #[must_use]
    pub fn plan(&self, mesh: MeshPolicy) -> RoomPlan {
        RoomPlan::resolve(self.channel_media, mesh)
    }
}

/// What a member may do in a channel of a community.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelGrant {
    pub community_id: CommunityId,
    pub kind: ChannelKind,
    pub permissions: Permissions,
}

impl CommunityService {
    /// Admits the user to a voice channel: 404 for an unknown channel, refused
    /// without membership, `view_channel` or `connect`, or for a text channel.
    pub async fn admit_voice(
        &self,
        channel_id: &ChannelId,
        user: &User,
    ) -> Result<VoiceGrant, CommunitiesError> {
        let seat = self.seat_by_channel(channel_id, user).await?;
        let channel = seat.state.channel(channel_id).ok_or(CommunitiesError::NotFound)?;
        let permissions = seat.state.admit_voice(&seat.standing, channel)?;
        Ok(VoiceGrant {
            community_id: seat.state.id().clone(),
            channel_media: channel.media,
            permissions,
        })
    }

    /// Whether a user in a voice channel may stay. A channel that is gone, or a
    /// membership or permission that is, means no; a store that cannot be asked is
    /// an error, and the caller lets the user stay.
    pub async fn may_stay(
        &self,
        channel_id: &ChannelId,
        user: &User,
    ) -> Result<bool, CommunitiesError> {
        match self.admit_voice(channel_id, user).await {
            Ok(_) => Ok(true),
            Err(CommunitiesError::Store(error)) => Err(CommunitiesError::Store(error)),
            Err(_) => Ok(false),
        }
    }

    /// What the user may do in a channel right now.
    pub async fn channel_permissions(
        &self,
        channel_id: &ChannelId,
        user: &User,
    ) -> Result<ChannelGrant, CommunitiesError> {
        let seat = self.seat_by_channel(channel_id, user).await?;
        grant_in(&seat, channel_id)
    }

    /// Like [`Self::admit_voice`], and also 409 when the channel already holds as
    /// many people as it admits, by the community's roster. A probe for clients
    /// whose WebSocket upgrade was refused; the roster may lag a moment behind.
    pub async fn probe_voice(
        &self,
        channel_id: &ChannelId,
        user: &User,
    ) -> Result<(), CommunitiesError> {
        let grant = self.admit_voice(channel_id, user).await?;
        let Some(capacity) = grant.plan(self.mesh).capacity() else {
            return Ok(());
        };
        let roster = self.roster.list(&grant.community_id).await?;
        let present = roster.iter().filter(|entry| entry.channel_id == *channel_id).count();
        if present >= capacity {
            return Err(CommunitiesError::Full);
        }
        Ok(())
    }

    /// May the user post in this text channel? Needs `view_channel` and
    /// `send_messages`; a voice channel is the wrong kind.
    pub async fn text_grant(
        &self,
        channel_id: &ChannelId,
        user: &User,
    ) -> Result<ChannelGrant, CommunitiesError> {
        let grant = self.channel_permissions(channel_id, user).await?;
        if grant.kind != ChannelKind::Text {
            return Err(CommunitiesError::WrongKind);
        }
        require(grant.permissions, Permissions::SEND_MESSAGES)?;
        Ok(grant)
    }

    /// May the user read the channel's history? Needs `view_channel` and `read_history`.
    pub async fn history_grant(
        &self,
        channel_id: &ChannelId,
        user: &User,
    ) -> Result<ChannelGrant, CommunitiesError> {
        let grant = self.channel_permissions(channel_id, user).await?;
        require(grant.permissions, Permissions::READ_HISTORY)?;
        Ok(grant)
    }

    /// What the user's feed of a community may carry: the channels they may view.
    pub async fn feed_filter(
        &self,
        id: &CommunityId,
        user: &User,
    ) -> Result<FeedFilter, CommunitiesError> {
        let seat = self.seat(id, user).await?;
        let visible = seat.state.visible_channels(&seat.standing);
        let channels = visible.into_iter().map(|(channel, _)| channel.id.clone()).collect();
        Ok(FeedFilter::new(user.id.clone(), channels))
    }

    /// Who is in the community's voice channels right now, across instances.
    pub async fn voice_roster(
        &self,
        id: &CommunityId,
    ) -> Result<Vec<VoiceEntry>, CommunitiesError> {
        Ok(self.roster.list(id).await?)
    }
}

fn grant_in(seat: &Seat, channel_id: &ChannelId) -> Result<ChannelGrant, CommunitiesError> {
    let channel = seat.state.channel(channel_id).ok_or(CommunitiesError::NotFound)?;
    let permissions = seat.state.channel_permissions(&seat.standing, channel);
    if permissions.is_empty() {
        return Err(CommunitiesError::Forbidden);
    }
    Ok(ChannelGrant { community_id: seat.state.id().clone(), kind: channel.kind, permissions })
}

fn require(permissions: Permissions, needed: Permissions) -> Result<(), Refusal> {
    if permissions.contains(needed) {
        return Ok(());
    }
    Err(Refusal::Missing(needed.name()))
}
