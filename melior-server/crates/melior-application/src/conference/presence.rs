//! Who is in which voice channel, as the conference reports it to the rest of the
//! community. The conference only queues reports: nothing under a room lock waits
//! on the store or the event bus behind the sink.

use melior_domain::{ChannelId, CommunityId, RoomPlan, SessionId, UserId, VoiceState};

/// What a join needs besides the user: the channel, how its room runs its media,
/// and where to report presence (none outside a community, as in some tests).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Admission {
    pub channel_id: ChannelId,
    pub plan: RoomPlan,
    pub presence: Option<PresenceContext>,
}

impl Admission {
    /// A channel whose presence is reported nowhere.
    #[must_use]
    pub fn unlisted(channel_id: ChannelId, plan: RoomPlan) -> Self {
        Self { channel_id, plan, presence: None }
    }
}

/// The community a participant's presence is reported to, and as whom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresenceContext {
    pub community_id: CommunityId,
    pub user_id: UserId,
}

/// One participant of a voice channel, as the community sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceEntry {
    pub channel_id: ChannelId,
    pub session_id: SessionId,
    pub user_id: UserId,
    pub username: String,
    pub voice: VoiceState,
}

/// A change in a community's voice presence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PresenceUpdate {
    Joined {
        community_id: CommunityId,
        entry: VoiceEntry,
    },
    Updated {
        community_id: CommunityId,
        entry: VoiceEntry,
    },
    Left {
        community_id: CommunityId,
        channel_id: ChannelId,
        session_id: SessionId,
    },
    /// Still there: keeps the entry alive in a store that expires it.
    Refreshed {
        community_id: CommunityId,
        entry: VoiceEntry,
    },
}

impl PresenceUpdate {
    #[must_use]
    pub fn joined(community_id: CommunityId, entry: VoiceEntry) -> Self {
        Self::Joined { community_id, entry }
    }

    #[must_use]
    pub fn updated(community_id: CommunityId, entry: VoiceEntry) -> Self {
        Self::Updated { community_id, entry }
    }

    #[must_use]
    pub fn refreshed(community_id: CommunityId, entry: VoiceEntry) -> Self {
        Self::Refreshed { community_id, entry }
    }

    /// The community the update is about.
    #[must_use]
    pub fn community_id(&self) -> &CommunityId {
        match self {
            Self::Joined { community_id, .. }
            | Self::Updated { community_id, .. }
            | Self::Left { community_id, .. }
            | Self::Refreshed { community_id, .. } => community_id,
        }
    }
}

/// Takes presence reports. Must not block: it is called under room locks.
pub trait PresenceSink: Send + Sync {
    fn report(&self, update: PresenceUpdate);
}

/// A [`PresenceSink`] that drops every report.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoPresence;

impl PresenceSink for NoPresence {
    fn report(&self, _update: PresenceUpdate) {}
}
