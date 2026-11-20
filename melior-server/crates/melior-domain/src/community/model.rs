//! The parts of a community: the community itself, members, roles, categories,
//! channels and permission overrides.

use super::{CategoryId, CommunityError, CommunityId, Name, Permissions, RoleId, Topic};
use crate::conference::{ChannelId, ChannelMedia, MeshPolicy, RoomPlan};
use crate::identity::UserId;
use std::collections::BTreeSet;

/// A community (what the UI calls a server).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Community {
    pub id: CommunityId,
    pub name: Name,
    pub owner_id: UserId,
    pub owner_username: String,
}

/// Someone who joined a community. `role_ids` never holds `@everyone`: every
/// member has it implicitly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    pub user_id: UserId,
    pub username: String,
    pub role_ids: BTreeSet<RoleId>,
}

impl Member {
    /// A member without roles.
    #[must_use]
    pub fn new(user_id: UserId, username: impl Into<String>) -> Self {
        Self { user_id, username: username.into(), role_ids: BTreeSet::new() }
    }
}

/// A named set of permissions with a place in the hierarchy. `@everyone` has
/// position 0; every other role has a position of at least 1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Role {
    pub id: RoleId,
    pub name: Name,
    /// `0xRRGGBB`.
    pub color: Option<u32>,
    pub position: u32,
    pub permissions: Permissions,
}

impl Role {
    pub const EVERYONE_NAME: &'static str = "@everyone";
    pub const COLOR_MAX: u32 = 0x00FF_FFFF;

    /// A colour as a client sends it: none, or `0` to `0xFFFFFF`.
    pub fn check_color(color: Option<u32>) -> Result<Option<u32>, CommunityError> {
        match color {
            Some(value) if value > Self::COLOR_MAX => Err(CommunityError::InvalidColor),
            other => Ok(other),
        }
    }
}

/// Whom a permission override is for.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum OverrideTarget {
    Role(RoleId),
    Member(UserId),
}

/// Permissions a category or channel allows or denies to one role or member, on
/// top of what the community grants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Override {
    pub target: OverrideTarget,
    pub allow: Permissions,
    pub deny: Permissions,
}

/// A group of channels whose overrides its synced channels share.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Category {
    pub id: CategoryId,
    pub name: Name,
    pub position: u32,
    pub overrides: Vec<Override>,
}

/// What a channel is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChannelKind {
    /// Video and chat: a conference room.
    Voice,
    /// Messages only.
    Text,
}

impl ChannelKind {
    /// The wire name: `voice` or `text`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Voice => "voice",
            Self::Text => "text",
        }
    }

    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "voice" => Some(Self::Voice),
            "text" => Some(Self::Text),
            _ => None,
        }
    }
}

/// A channel of a community. Its id is the conference room's key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Channel {
    pub id: ChannelId,
    pub name: Name,
    pub topic: Topic,
    pub kind: ChannelKind,
    pub category_id: Option<CategoryId>,
    pub position: u32,
    /// Whether the channel uses its category's overrides instead of its own.
    pub synced: bool,
    pub overrides: Vec<Override>,
    /// Always the default for a text channel.
    pub media: ChannelMedia,
}

impl Channel {
    pub const USER_LIMIT_MIN: u8 = 2;
    pub const USER_LIMIT_MAX: u8 = 99;
    /// The largest peer-to-peer channel that may be saved while mesh is off.
    pub const MESH_LIMIT_MAX: usize = 8;

    /// A user limit as a client sends it: none, or 2 to 99.
    pub fn check_user_limit(limit: Option<u32>) -> Result<Option<u8>, CommunityError> {
        let Some(limit) = limit else {
            return Ok(None);
        };
        u8::try_from(limit)
            .ok()
            .filter(|limit| (Self::USER_LIMIT_MIN..=Self::USER_LIMIT_MAX).contains(limit))
            .map(Some)
            .ok_or(CommunityError::InvalidUserLimit)
    }

    /// Whether `media` fits a channel of `kind` on an instance with `policy`. A text
    /// channel has none; a peer-to-peer channel may not ask for more people than a
    /// mesh holds (8 when mesh is off here, since another instance may run one).
    pub fn check_media(
        kind: ChannelKind,
        media: ChannelMedia,
        policy: MeshPolicy,
    ) -> Result<(), CommunityError> {
        if kind == ChannelKind::Text && media != ChannelMedia::default() {
            return Err(CommunityError::TextChannelMedia);
        }
        if media.mode != crate::conference::MediaMode::PeerToPeer {
            return Ok(());
        }
        let max = if policy.is_on() { policy.limit() } else { Self::MESH_LIMIT_MAX };
        match media.user_limit {
            Some(limit) if usize::from(limit) > max => Err(CommunityError::PeerToPeerLimit(max)),
            _ => Ok(()),
        }
    }

    /// What the channel's room does with its media on this instance.
    #[must_use]
    pub fn plan(&self, policy: MeshPolicy) -> RoomPlan {
        RoomPlan::resolve(self.media, policy)
    }

    /// Whether a conference may run in it.
    #[must_use]
    pub fn is_voice(&self) -> bool {
        self.kind == ChannelKind::Voice
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conference::MediaMode;

    fn media(mode: MediaMode, user_limit: Option<u8>) -> ChannelMedia {
        ChannelMedia { mode, user_limit }
    }

    #[test]
    fn a_colour_is_at_most_0xffffff() {
        assert_eq!(Role::check_color(None), Ok(None));
        assert_eq!(Role::check_color(Some(0xFF_FFFF)), Ok(Some(0xFF_FFFF)));
        assert_eq!(Role::check_color(Some(0x100_0000)), Err(CommunityError::InvalidColor));
    }

    #[test]
    fn a_user_limit_is_two_to_ninety_nine() {
        assert_eq!(Channel::check_user_limit(None), Ok(None));
        assert_eq!(Channel::check_user_limit(Some(2)), Ok(Some(2)));
        assert_eq!(Channel::check_user_limit(Some(99)), Ok(Some(99)));
        for bad in [0, 1, 100, 256, u32::MAX] {
            assert_eq!(Channel::check_user_limit(Some(bad)), Err(CommunityError::InvalidUserLimit));
        }
    }

    #[test]
    fn a_text_channel_has_no_media_settings() {
        let on = MeshPolicy::new(4);
        assert_eq!(Channel::check_media(ChannelKind::Text, ChannelMedia::default(), on), Ok(()));
        assert_eq!(
            Channel::check_media(ChannelKind::Text, media(MediaMode::Server, None), on),
            Err(CommunityError::TextChannelMedia)
        );
        assert_eq!(
            Channel::check_media(ChannelKind::Text, media(MediaMode::Auto, Some(5)), on),
            Err(CommunityError::TextChannelMedia)
        );
    }

    #[test]
    fn a_peer_to_peer_channel_cannot_exceed_the_mesh_limit() {
        let on = MeshPolicy::new(4);
        let voice = ChannelKind::Voice;
        assert_eq!(Channel::check_media(voice, media(MediaMode::PeerToPeer, Some(4)), on), Ok(()));
        assert_eq!(Channel::check_media(voice, media(MediaMode::PeerToPeer, None), on), Ok(()));
        assert_eq!(
            Channel::check_media(voice, media(MediaMode::PeerToPeer, Some(5)), on),
            Err(CommunityError::PeerToPeerLimit(4))
        );
        assert_eq!(Channel::check_media(voice, media(MediaMode::Auto, Some(99)), on), Ok(()));
        assert_eq!(Channel::check_media(voice, media(MediaMode::Server, Some(99)), on), Ok(()));
    }

    #[test]
    fn with_mesh_off_a_peer_to_peer_channel_may_ask_for_up_to_eight() {
        let off = MeshPolicy::OFF;
        let voice = ChannelKind::Voice;
        assert_eq!(Channel::check_media(voice, media(MediaMode::PeerToPeer, Some(8)), off), Ok(()));
        assert_eq!(
            Channel::check_media(voice, media(MediaMode::PeerToPeer, Some(9)), off),
            Err(CommunityError::PeerToPeerLimit(8))
        );
    }

    #[test]
    fn kinds_round_trip_their_wire_names() {
        for kind in [ChannelKind::Voice, ChannelKind::Text] {
            assert_eq!(ChannelKind::parse(kind.as_str()), Some(kind));
        }
        assert_eq!(ChannelKind::parse("stage"), None);
    }
}
