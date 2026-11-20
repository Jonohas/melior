//! Changes to roles and to who holds them: the hierarchy rules applied.

use super::{
    CommunityError, CommunityState, Member, Name, Permissions, Rank, Refusal, Rejected, Role,
    RoleId, Standing,
};
use std::collections::BTreeSet;

/// A new role as a client asks for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleDraft {
    pub name: Name,
    pub color: Option<u32>,
    pub permissions: Permissions,
}

/// Changes to a role; `None` leaves a field alone. `color: Some(None)` removes the colour.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RoleChanges {
    pub name: Option<Name>,
    pub color: Option<Option<u32>>,
    pub permissions: Option<Permissions>,
    pub position: Option<u32>,
}

impl CommunityState {
    /// A new role, placed just below the actor's rank: one above the highest role
    /// below it, but never at or above the actor's own top role (positions may be
    /// shared, so nothing else moves).
    pub fn new_role(&self, actor: &Standing, draft: RoleDraft) -> Result<Role, Rejected> {
        let color = Role::check_color(draft.color)?;
        actor.require(Permissions::MANAGE_ROLES)?;
        let position = self.position_below(actor.rank()).ok_or(Refusal::RoleNotBelow)?;
        actor.may_change_role(None, position, draft.permissions)?;
        Ok(Role {
            id: RoleId::generate(),
            name: draft.name,
            color,
            position,
            permissions: draft.permissions,
        })
    }

    fn position_below(&self, rank: Rank) -> Option<u32> {
        let highest_below = self
            .roles
            .iter()
            .map(|role| role.position)
            .filter(|position| rank.is_above(*position))
            .max()
            .unwrap_or(0);
        let next = highest_below.saturating_add(1);
        match rank {
            Rank::Owner => Some(next),
            Rank::Position(top) if top >= 2 => Some(next.min(top - 1)),
            Rank::Position(_) => None,
        }
    }

    /// The role with `changes` applied. `@everyone` keeps its name, colour and
    /// position; only its permissions change.
    pub fn edit_role(
        &self,
        actor: &Standing,
        id: &RoleId,
        changes: RoleChanges,
    ) -> Result<Role, Rejected> {
        let role = self.role(id).ok_or_else(|| CommunityError::UnknownRole(id.to_string()))?;
        let is_everyone = *id == self.everyone_id();
        let renames = changes.name.is_some() || changes.color.is_some();
        if is_everyone && (renames || changes.position.is_some()) {
            return Err(CommunityError::EveryoneRole.into());
        }
        if changes.position == Some(0) {
            return Err(CommunityError::InvalidPosition.into());
        }
        let color = Role::check_color(changes.color.unwrap_or(role.color))?;
        let permissions = changes.permissions.unwrap_or(role.permissions);
        let position = changes.position.unwrap_or(role.position);
        let added = permissions.difference(role.permissions);
        actor.may_change_role(Some(role.position), position, added)?;
        Ok(Role {
            id: role.id.clone(),
            name: changes.name.unwrap_or_else(|| role.name.clone()),
            color,
            position,
            permissions,
        })
    }

    /// Deleting a role: never `@everyone`, otherwise as editing it.
    pub fn delete_role(&self, actor: &Standing, id: &RoleId) -> Result<(), Rejected> {
        if *id == self.everyone_id() {
            return Err(CommunityError::EveryoneRole.into());
        }
        let role = self.role(id).ok_or_else(|| CommunityError::UnknownRole(id.to_string()))?;
        actor.may_change_role(Some(role.position), role.position, Permissions::NONE)?;
        Ok(())
    }

    /// `target` with exactly the roles `wanted`. Every role added or removed must be
    /// one the actor may assign; the roles that stay are not checked.
    pub fn assign_roles(
        &self,
        actor: &Standing,
        target: &Member,
        wanted: BTreeSet<RoleId>,
    ) -> Result<Member, Rejected> {
        if wanted.contains(&self.everyone_id()) {
            return Err(CommunityError::EveryoneRole.into());
        }
        let changed = wanted.symmetric_difference(&target.role_ids);
        for id in changed {
            let role = self.role(id).ok_or_else(|| CommunityError::UnknownRole(id.to_string()))?;
            actor.may_assign(role.position)?;
        }
        Ok(Member { role_ids: wanted, ..target.clone() })
    }

    /// Kicking `target` out of the community.
    pub fn kick(&self, actor: &Standing, target: &Member) -> Result<(), Rejected> {
        actor.may_kick(&self.standing(target))?;
        Ok(())
    }

    /// The actor leaving the community.
    pub fn leave(actor: &Standing) -> Result<(), Rejected> {
        actor.may_leave()?;
        Ok(())
    }
}
