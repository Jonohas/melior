//! The community aggregate: a community with its roles, categories and channels,
//! and the questions every use case asks of it. The changes live in `roles` and
//! `channels`.

use super::{
    Category, CategoryId, Channel, ChannelKind, Community, CommunityError, CommunityId, Invite,
    InviteSettings, Member, Name, Override, OverrideTarget, Permissions, Refusal, Rejected, Role,
    RoleId, Standing, Topic,
};
use crate::conference::{ChannelId, ChannelMedia};
use crate::identity::User;
use std::collections::BTreeSet;

/// A community and its structure. Members are not part of it: there may be many,
/// and each use case needs one or two.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommunityState {
    pub community: Community,
    pub roles: Vec<Role>,
    pub categories: Vec<Category>,
    pub channels: Vec<Channel>,
}

impl CommunityState {
    pub const TEXT_CATEGORY: &'static str = "Text channels";
    pub const VOICE_CATEGORY: &'static str = "Voice channels";
    pub const TEXT_CHANNEL: &'static str = "general";
    pub const VOICE_CHANNEL: &'static str = "General";

    /// A new community owned by `owner`, who is its first member. It starts with
    /// `@everyone` (the default permissions), a category `Text channels` holding
    /// the text channel `general`, and a category `Voice channels` holding the
    /// voice channel `General`.
    #[must_use]
    pub fn create(id: CommunityId, name: Name, owner: &User) -> (Self, Member) {
        let everyone = Role {
            id: RoleId::everyone(&id),
            name: Name(Role::EVERYONE_NAME.to_owned()),
            color: None,
            position: 0,
            permissions: Permissions::EVERYONE_DEFAULT,
        };
        let text = seed_category(Self::TEXT_CATEGORY, 0);
        let voice = seed_category(Self::VOICE_CATEGORY, 1);
        let channels = vec![
            seed_channel(Self::TEXT_CHANNEL, ChannelKind::Text, &text, 0),
            seed_channel(Self::VOICE_CHANNEL, ChannelKind::Voice, &voice, 1),
        ];
        let community = Community {
            id,
            name,
            owner_id: owner.id.clone(),
            owner_username: owner.username.clone(),
        };
        let state =
            Self { community, roles: vec![everyone], categories: vec![text, voice], channels };
        (state, Member::new(owner.id.clone(), owner.username.clone()))
    }

    #[must_use]
    pub fn id(&self) -> &CommunityId {
        &self.community.id
    }

    #[must_use]
    pub fn everyone_id(&self) -> RoleId {
        RoleId::everyone(&self.community.id)
    }

    #[must_use]
    pub fn role(&self, id: &RoleId) -> Option<&Role> {
        self.roles.iter().find(|role| role.id == *id)
    }

    #[must_use]
    pub fn category(&self, id: &CategoryId) -> Option<&Category> {
        self.categories.iter().find(|category| category.id == *id)
    }

    #[must_use]
    pub fn channel(&self, id: &ChannelId) -> Option<&Channel> {
        self.channels.iter().find(|channel| channel.id == *id)
    }

    #[must_use]
    pub fn standing(&self, member: &Member) -> Standing {
        Standing::new(&self.community, &self.roles, member)
    }

    /// What `standing`'s member may do in `channel`.
    #[must_use]
    pub fn channel_permissions(&self, standing: &Standing, channel: &Channel) -> Permissions {
        let category = channel.category_id.as_ref().and_then(|id| self.category(id));
        standing.in_channel(channel, category)
    }

    /// The channels the member may see, with what they may do there, by position.
    #[must_use]
    pub fn visible_channels(&self, standing: &Standing) -> Vec<(&Channel, Permissions)> {
        let mut visible: Vec<(&Channel, Permissions)> = self
            .channels
            .iter()
            .map(|channel| (channel, self.channel_permissions(standing, channel)))
            .filter(|(_, permissions)| permissions.contains(Permissions::VIEW_CHANNEL))
            .collect();
        visible.sort_by(|(a, _), (b, _)| (a.position, &a.id).cmp(&(b.position, &b.id)));
        visible
    }

    /// Whether the member may join the conference in a channel: it must be a voice
    /// channel they may see and connect to. Returns what they may do there.
    pub fn admit_voice(
        &self,
        standing: &Standing,
        channel: &Channel,
    ) -> Result<Permissions, Refusal> {
        if !channel.is_voice() {
            return Err(Refusal::NotVoice);
        }
        let permissions = self.channel_permissions(standing, channel);
        for needed in [Permissions::VIEW_CHANNEL, Permissions::CONNECT] {
            if !permissions.contains(needed) {
                return Err(Refusal::Missing(needed.name()));
            }
        }
        Ok(permissions)
    }

    /// The community with a new name: the owner or an administrator.
    pub fn rename(&self, actor: &Standing, name: Name) -> Result<Community, Rejected> {
        actor.may_rename_community()?;
        Ok(Community { name, ..self.community.clone() })
    }

    /// A new invite by `creator`: needs `create_invite`.
    pub fn new_invite(
        &self,
        actor: &Standing,
        creator: &Member,
        settings: InviteSettings,
        now_ms: i64,
    ) -> Result<Invite, Rejected> {
        let settings = settings.check()?;
        actor.require(Permissions::CREATE_INVITE)?;
        Ok(Invite::new(self.community.id.clone(), creator, settings, now_ms))
    }

    /// Listing the invites: whoever manages channels (administrators included).
    pub fn may_list_invites(actor: &Standing) -> Result<(), Refusal> {
        actor.require(Permissions::MANAGE_CHANNELS)
    }

    /// Overrides as a client sends them: each role or member at most once, and
    /// every role one of this community's.
    pub fn check_overrides(&self, overrides: &[Override]) -> Result<(), CommunityError> {
        let mut seen = BTreeSet::new();
        for entry in overrides {
            if !seen.insert(&entry.target) {
                return Err(CommunityError::InvalidOverride);
            }
            if let OverrideTarget::Role(id) = &entry.target
                && self.role(id).is_none()
            {
                return Err(CommunityError::UnknownRole(id.to_string()));
            }
        }
        Ok(())
    }

    /// The next free position after every existing channel.
    pub(super) fn next_channel_position(&self) -> u32 {
        self.channels.iter().map(|c| c.position.saturating_add(1)).max().unwrap_or(0)
    }

    pub(super) fn next_category_position(&self) -> u32 {
        self.categories.iter().map(|c| c.position.saturating_add(1)).max().unwrap_or(0)
    }
}

fn seed_category(name: &str, position: u32) -> Category {
    Category {
        id: CategoryId::generate(),
        name: Name(name.to_owned()),
        position,
        overrides: Vec::new(),
    }
}

fn seed_channel(name: &str, kind: ChannelKind, category: &Category, position: u32) -> Channel {
    Channel {
        id: ChannelId::generate(),
        name: Name(name.to_owned()),
        topic: Topic::default(),
        kind,
        category_id: Some(category.id.clone()),
        position,
        synced: true,
        overrides: Vec::new(),
        media: ChannelMedia::default(),
    }
}
