//! The community API's response bodies, and overrides in both directions.

use super::support::Failure;
use melior_application::{ChannelView, CommunityView, InvitePreview, MemberView};
use melior_domain::{
    Category, ChannelKind, Community, Invite, Member, Override, OverrideTarget, Permissions, Role,
    RoleId, UserId,
};
use serde::{Deserialize, Serialize};

/// The longest user id an override may name.
const MAX_USER_ID_LEN: usize = 255;

#[derive(Debug, Serialize)]
pub(super) struct CommunitySummaryDto {
    id: String,
    name: String,
    owner_username: String,
}

impl From<Community> for CommunitySummaryDto {
    fn from(community: Community) -> Self {
        Self {
            id: community.id.to_string(),
            name: community.name.to_string(),
            owner_username: community.owner_username,
        }
    }
}

#[derive(Debug, Serialize)]
pub(super) struct CommunityViewDto {
    id: String,
    name: String,
    owner_id: String,
    owner_username: String,
    me: MeDto,
    roles: Vec<RoleDto>,
    categories: Vec<CategoryDto>,
    channels: Vec<ChannelViewDto>,
}

impl From<CommunityView> for CommunityViewDto {
    fn from(view: CommunityView) -> Self {
        Self {
            id: view.community.id.to_string(),
            name: view.community.name.to_string(),
            owner_id: view.community.owner_id.to_string(),
            owner_username: view.community.owner_username,
            me: view.me.into(),
            roles: view.roles.into_iter().map(Into::into).collect(),
            categories: view.categories.into_iter().map(Into::into).collect(),
            channels: view.channels.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Serialize)]
struct MeDto {
    user_id: String,
    role_ids: Vec<String>,
    permissions: Vec<&'static str>,
    /// The highest position of the member's roles; `u32::MAX` for the owner.
    rank: u32,
    is_owner: bool,
}

impl From<MemberView> for MeDto {
    fn from(me: MemberView) -> Self {
        Self {
            user_id: me.member.user_id.to_string(),
            role_ids: me.member.role_ids.iter().map(ToString::to_string).collect(),
            permissions: me.permissions.names(),
            rank: me.rank.value(),
            is_owner: me.is_owner,
        }
    }
}

#[derive(Debug, Serialize)]
pub(super) struct RoleDto {
    id: String,
    name: String,
    color: Option<u32>,
    position: u32,
    permissions: Vec<&'static str>,
}

impl From<Role> for RoleDto {
    fn from(role: Role) -> Self {
        Self {
            id: role.id.to_string(),
            name: role.name.to_string(),
            color: role.color,
            position: role.position,
            permissions: role.permissions.names(),
        }
    }
}

#[derive(Debug, Serialize)]
pub(super) struct CategoryDto {
    id: String,
    name: String,
    position: u32,
    overrides: Vec<OverrideDto>,
}

impl From<Category> for CategoryDto {
    fn from(category: Category) -> Self {
        Self {
            id: category.id.to_string(),
            name: category.name.to_string(),
            position: category.position,
            overrides: category.overrides.iter().map(Into::into).collect(),
        }
    }
}

/// `{ "target": { "kind": "role" | "member", "id": ... }, "allow": [...], "deny": [...] }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct OverrideDto {
    target: TargetDto,
    #[serde(default)]
    allow: Vec<String>,
    #[serde(default)]
    deny: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct TargetDto {
    kind: TargetKind,
    id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum TargetKind {
    Role,
    Member,
}

impl From<&Override> for OverrideDto {
    fn from(entry: &Override) -> Self {
        let target = match &entry.target {
            OverrideTarget::Role(id) => TargetDto { kind: TargetKind::Role, id: id.to_string() },
            OverrideTarget::Member(id) => {
                TargetDto { kind: TargetKind::Member, id: id.to_string() }
            }
        };
        let names = |set: Permissions| set.names().into_iter().map(str::to_owned).collect();
        Self { target, allow: names(entry.allow), deny: names(entry.deny) }
    }
}

impl OverrideDto {
    /// The override as the domain takes it: a role id or a plausible user id, and
    /// known permission names only.
    pub(super) fn into_override(self) -> Result<Override, Failure> {
        let target = match self.target.kind {
            TargetKind::Role => OverrideTarget::Role(
                RoleId::parse(&self.target.id)
                    .ok_or_else(|| Failure::Bad("invalid role id in override".into()))?,
            ),
            TargetKind::Member => {
                let id = self.target.id;
                if id.is_empty() || id.len() > MAX_USER_ID_LEN {
                    return Err(Failure::Bad("invalid user id in override".into()));
                }
                OverrideTarget::Member(UserId::new(id))
            }
        };
        Ok(Override {
            target,
            allow: Permissions::from_names(&self.allow)?,
            deny: Permissions::from_names(&self.deny)?,
        })
    }
}

#[derive(Debug, Serialize)]
pub(super) struct ChannelViewDto {
    id: String,
    community_id: String,
    name: String,
    topic: String,
    kind: &'static str,
    category_id: Option<String>,
    position: u32,
    synced: bool,
    overrides: Vec<OverrideDto>,
    media_mode: &'static str,
    user_limit: Option<u8>,
    effective_mode: &'static str,
    effective_limit: Option<usize>,
    media_warning: Option<&'static str>,
    my_permissions: Vec<&'static str>,
}

impl From<ChannelView> for ChannelViewDto {
    fn from(view: ChannelView) -> Self {
        let channel = view.channel;
        // A text channel has no room: it reports its settings as they are.
        let text = channel.kind == ChannelKind::Text;
        let effective_mode =
            if text { channel.media.mode.as_str() } else { view.plan.effective_mode().as_str() };
        Self {
            id: channel.id.to_string(),
            community_id: view.community_id.to_string(),
            name: channel.name.to_string(),
            topic: channel.topic.as_str().to_owned(),
            kind: channel.kind.as_str(),
            category_id: channel.category_id.map(|id| id.to_string()),
            position: channel.position,
            synced: channel.synced,
            overrides: channel.overrides.iter().map(Into::into).collect(),
            media_mode: channel.media.mode.as_str(),
            user_limit: channel.media.user_limit,
            effective_mode,
            effective_limit: if text { None } else { view.plan.capacity() },
            media_warning: if text {
                None
            } else {
                view.plan.warning().map(melior_domain::MediaWarning::as_str)
            },
            my_permissions: view.permissions.names(),
        }
    }
}

#[derive(Debug, Serialize)]
pub(super) struct MemberDto {
    user_id: String,
    username: String,
    role_ids: Vec<String>,
}

impl From<Member> for MemberDto {
    fn from(member: Member) -> Self {
        Self {
            user_id: member.user_id.to_string(),
            role_ids: member.role_ids.iter().map(ToString::to_string).collect(),
            username: member.username,
        }
    }
}

#[derive(Debug, Serialize)]
pub(super) struct InviteDto {
    code: String,
    community_id: String,
    created_by: String,
    expires_at_ms: Option<i64>,
    max_uses: Option<u32>,
    uses: u32,
}

impl From<Invite> for InviteDto {
    fn from(invite: Invite) -> Self {
        Self {
            code: invite.code.to_string(),
            community_id: invite.community_id.to_string(),
            created_by: invite.created_by,
            expires_at_ms: invite.expires_at_ms,
            max_uses: invite.max_uses,
            uses: invite.uses,
        }
    }
}

#[derive(Debug, Serialize)]
pub(super) struct InvitePreviewDto {
    code: String,
    community: CommunityRefDto,
    member_count: u64,
    usable: bool,
}

#[derive(Debug, Serialize)]
struct CommunityRefDto {
    id: String,
    name: String,
}

impl From<InvitePreview> for InvitePreviewDto {
    fn from(preview: InvitePreview) -> Self {
        Self {
            code: preview.invite.code.to_string(),
            community: CommunityRefDto {
                id: preview.community.id.to_string(),
                name: preview.community.name.to_string(),
            },
            member_count: preview.member_count,
            usable: preview.usable,
        }
    }
}

/// `{ "community_id": ... }`: where an invite led.
#[derive(Debug, Serialize)]
pub(super) struct JoinedDto {
    pub(super) community_id: String,
}

#[cfg(test)]
#[path = "community_dto_tests.rs"]
mod tests;
