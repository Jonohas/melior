//! A member's standing in a community: what they may do there and where they are
//! in the hierarchy. Holds the permission resolution and the hierarchy rules.

use super::{
    Category, Channel, Community, Member, Override, OverrideTarget, Permissions, Refusal, Role,
    RoleId,
};
use crate::identity::UserId;
use std::collections::BTreeSet;

/// Where a member is in the hierarchy: the highest position of their roles, or the
/// owner, who outranks everyone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rank {
    Position(u32),
    Owner,
}

impl Rank {
    /// The number clients compare role positions with; the owner's is `u32::MAX`.
    #[must_use]
    pub fn value(self) -> u32 {
        match self {
            Self::Position(position) => position,
            Self::Owner => u32::MAX,
        }
    }

    /// Whether a role at `position` is below this rank.
    #[must_use]
    pub fn is_above(self, position: u32) -> bool {
        self > Self::Position(position)
    }
}

/// A member's community-wide permissions and rank.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Standing {
    user_id: UserId,
    everyone: RoleId,
    role_ids: BTreeSet<RoleId>,
    is_owner: bool,
    base: Permissions,
    rank: Rank,
}

impl Standing {
    /// Steps 1 to 3 of the resolution: the owner and administrators have every
    /// permission; everyone else has `@everyone`'s plus their roles'.
    #[must_use]
    pub fn new(community: &Community, roles: &[Role], member: &Member) -> Self {
        let everyone = RoleId::everyone(&community.id);
        let is_owner = member.user_id == community.owner_id;
        let held: Vec<&Role> = roles
            .iter()
            .filter(|role| role.id == everyone || member.role_ids.contains(&role.id))
            .collect();
        let granted = held.iter().fold(Permissions::NONE, |set, role| set.union(role.permissions));
        let base = if is_owner || granted.contains(Permissions::ADMINISTRATOR) {
            Permissions::ALL
        } else {
            granted
        };
        let top = held.iter().map(|role| role.position).max().unwrap_or(0);
        let rank = if is_owner { Rank::Owner } else { Rank::Position(top) };
        Self {
            user_id: member.user_id.clone(),
            everyone,
            role_ids: member.role_ids.clone(),
            is_owner,
            base,
            rank,
        }
    }

    #[must_use]
    pub fn user_id(&self) -> &UserId {
        &self.user_id
    }

    #[must_use]
    pub fn is_owner(&self) -> bool {
        self.is_owner
    }

    /// The community-wide permissions (steps 1 to 3).
    #[must_use]
    pub fn base(&self) -> Permissions {
        self.base
    }

    #[must_use]
    pub fn rank(&self) -> Rank {
        self.rank
    }

    #[must_use]
    pub fn role_ids(&self) -> &BTreeSet<RoleId> {
        &self.role_ids
    }

    /// What the member may do in `channel`, whose category is `category` (steps 1 to 6).
    #[must_use]
    pub fn in_channel(&self, channel: &Channel, category: Option<&Category>) -> Permissions {
        if self.base.contains(Permissions::ADMINISTRATOR) {
            return Permissions::ALL;
        }
        let overrides = match category {
            Some(category) if channel.synced && channel.category_id.is_some() => {
                &category.overrides
            }
            _ => &channel.overrides,
        };
        let resolved = self.apply(overrides);
        if !resolved.contains(Permissions::VIEW_CHANNEL) {
            return Permissions::NONE;
        }
        resolved
    }

    /// Step 5: `@everyone`'s override, then the union of the member's roles', then
    /// the member's own; each removes its deny and adds its allow.
    fn apply(&self, overrides: &[Override]) -> Permissions {
        let mut role_allow = Permissions::NONE;
        let mut role_deny = Permissions::NONE;
        let mut everyone = (Permissions::NONE, Permissions::NONE);
        let mut own = (Permissions::NONE, Permissions::NONE);
        for entry in overrides {
            match &entry.target {
                OverrideTarget::Role(id) if *id == self.everyone => {
                    everyone = (entry.allow, entry.deny);
                }
                OverrideTarget::Role(id) if self.role_ids.contains(id) => {
                    role_allow = role_allow.union(entry.allow);
                    role_deny = role_deny.union(entry.deny);
                }
                OverrideTarget::Member(id) if *id == self.user_id => {
                    own = (entry.allow, entry.deny);
                }
                OverrideTarget::Role(_) | OverrideTarget::Member(_) => {}
            }
        }
        [everyone, (role_allow, role_deny), own]
            .into_iter()
            .fold(self.base, |set, (allow, deny)| set.difference(deny).union(allow))
    }

    /// A community-wide permission, from the base permissions.
    pub fn require(&self, permission: Permissions) -> Result<(), Refusal> {
        if self.base.contains(permission) {
            return Ok(());
        }
        Err(Refusal::Missing(permission.name()))
    }

    /// Renaming a community: the owner or an administrator.
    pub fn may_rename_community(&self) -> Result<(), Refusal> {
        self.require(Permissions::ADMINISTRATOR)
    }

    /// Deleting a community: the owner alone.
    pub fn may_delete_community(&self) -> Result<(), Refusal> {
        if self.is_owner {
            return Ok(());
        }
        Err(Refusal::OwnerOnly)
    }

    /// Creating, editing or deleting a role at `old` (none when creating) and
    /// `new` position that grants `added` permissions it did not grant before:
    /// needs `manage_roles`, both positions below the actor's rank, and only
    /// permissions the actor has itself (unless owner or administrator).
    pub fn may_change_role(
        &self,
        old: Option<u32>,
        new: u32,
        added: Permissions,
    ) -> Result<(), Refusal> {
        self.require(Permissions::MANAGE_ROLES)?;
        let below = old.is_none_or(|old| self.rank.is_above(old)) && self.rank.is_above(new);
        if !below {
            return Err(Refusal::RoleNotBelow);
        }
        if !self.base.contains(Permissions::ADMINISTRATOR) && !self.base.contains(added) {
            return Err(Refusal::ExceedsOwnPermissions);
        }
        Ok(())
    }

    /// Assigning or unassigning a role at `position` to a member.
    pub fn may_assign(&self, position: u32) -> Result<(), Refusal> {
        self.require(Permissions::MANAGE_ROLES)?;
        if !self.rank.is_above(position) {
            return Err(Refusal::RoleNotBelow);
        }
        Ok(())
    }

    /// Kicking `target`: needs `kick_members`; never the owner; only someone ranked below.
    pub fn may_kick(&self, target: &Standing) -> Result<(), Refusal> {
        self.require(Permissions::KICK_MEMBERS)?;
        if target.is_owner {
            return Err(Refusal::CannotKickOwner);
        }
        if self.rank <= target.rank {
            return Err(Refusal::TargetOutranks);
        }
        Ok(())
    }

    /// Leaving: anyone but the owner, who may only delete the community.
    pub fn may_leave(&self) -> Result<(), Refusal> {
        if self.is_owner {
            return Err(Refusal::OwnerCannotLeave);
        }
        Ok(())
    }
}
