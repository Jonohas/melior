//! Members and roles: who belongs, who holds which role, and the roles themselves.

use super::{CommunitiesError, CommunityService};
use crate::communities::ports::{CommunityEvent, RemovalReason};
use melior_domain::{
    CommunityId, CommunityState, Member, Role, RoleChanges, RoleDraft, RoleId, User, UserId,
};
use std::collections::BTreeSet;
use tracing::info;

impl CommunityService {
    /// The members of a community, for its members.
    pub async fn members(
        &self,
        id: &CommunityId,
        user: &User,
    ) -> Result<Vec<Member>, CommunitiesError> {
        self.seat(id, user).await?;
        Ok(self.store.members(id).await?)
    }

    /// Kicks `target` out, or lets the user leave when `target` is the user.
    #[tracing::instrument(name = "communities.remove_member", skip_all, fields(community = %id))]
    pub async fn remove_member(
        &self,
        id: &CommunityId,
        user: &User,
        target: &UserId,
    ) -> Result<(), CommunitiesError> {
        let seat = self.seat(id, user).await?;
        let reason = if *target == user.id {
            CommunityState::leave(&seat.standing)?;
            RemovalReason::Left
        } else {
            let member = self.store.member(id, target).await?.ok_or(CommunitiesError::NotFound)?;
            seat.state.kick(&seat.standing, &member)?;
            RemovalReason::Kicked
        };
        self.store.remove_member(id, target).await?;
        info!(community = %id, user = %user.username, target = %target, ?reason, "member removed");
        let event = CommunityEvent::MemberRemoved { user_id: target.clone(), reason };
        self.publish(id, event).await;
        Ok(())
    }

    /// Replaces a member's roles.
    pub async fn set_member_roles(
        &self,
        id: &CommunityId,
        user: &User,
        target: &UserId,
        role_ids: BTreeSet<RoleId>,
    ) -> Result<Member, CommunitiesError> {
        let seat = self.seat(id, user).await?;
        let member = self.store.member(id, target).await?.ok_or(CommunitiesError::NotFound)?;
        let member = seat.state.assign_roles(&seat.standing, &member, role_ids)?;
        self.store.set_member_roles(id, &member).await?;
        self.publish(id, CommunityEvent::StructureChanged).await;
        Ok(member)
    }

    pub async fn create_role(
        &self,
        id: &CommunityId,
        user: &User,
        draft: RoleDraft,
    ) -> Result<Role, CommunitiesError> {
        let seat = self.seat(id, user).await?;
        let role = seat.state.new_role(&seat.standing, draft)?;
        self.store.save_role(id, &role).await?;
        self.publish(id, CommunityEvent::StructureChanged).await;
        Ok(role)
    }

    pub async fn edit_role(
        &self,
        id: &CommunityId,
        user: &User,
        role_id: &RoleId,
        changes: RoleChanges,
    ) -> Result<Role, CommunitiesError> {
        let seat = self.seat(id, user).await?;
        seat.state.role(role_id).ok_or(CommunitiesError::NotFound)?;
        let role = seat.state.edit_role(&seat.standing, role_id, changes)?;
        self.store.save_role(id, &role).await?;
        self.publish(id, CommunityEvent::StructureChanged).await;
        Ok(role)
    }

    pub async fn delete_role(
        &self,
        id: &CommunityId,
        user: &User,
        role_id: &RoleId,
    ) -> Result<(), CommunitiesError> {
        let seat = self.seat(id, user).await?;
        seat.state.role(role_id).ok_or(CommunitiesError::NotFound)?;
        seat.state.delete_role(&seat.standing, role_id)?;
        self.store.delete_role(id, role_id).await?;
        self.publish(id, CommunityEvent::StructureChanged).await;
        Ok(())
    }
}
