//! Invites: creating, listing, deleting, previewing and redeeming them.

use super::{CommunitiesError, CommunityService, now_ms, require_role};
use crate::communities::ports::{CommunityEvent, Redemption};
use melior_domain::{
    Community, CommunityId, CommunityState, Invite, InviteCode, InviteSettings, Member, User,
};
use tracing::info;

/// What someone holding an invite code may see before joining.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvitePreview {
    pub invite: Invite,
    pub community: Community,
    pub member_count: u64,
    pub usable: bool,
}

impl CommunityService {
    pub async fn create_invite(
        &self,
        id: &CommunityId,
        user: &User,
        settings: InviteSettings,
    ) -> Result<Invite, CommunitiesError> {
        let seat = self.seat(id, user).await?;
        let invite = seat.state.new_invite(&seat.standing, &seat.member, settings, now_ms())?;
        self.store.save_invite(&invite).await?;
        Ok(invite)
    }

    /// The community's invites: for whoever manages channels.
    pub async fn invites(
        &self,
        id: &CommunityId,
        user: &User,
    ) -> Result<Vec<Invite>, CommunitiesError> {
        let seat = self.seat(id, user).await?;
        CommunityState::may_list_invites(&seat.standing)?;
        Ok(self.store.invites(id).await?)
    }

    /// Deletes an invite: its creator, or whoever manages channels.
    pub async fn delete_invite(
        &self,
        code: &InviteCode,
        user: &User,
    ) -> Result<(), CommunitiesError> {
        require_role(user)?;
        let invite = self.store.invite(code).await?.ok_or(CommunitiesError::NotFound)?;
        let seat = self.seat(&invite.community_id, user).await?;
        invite.may_delete(&seat.standing)?;
        self.store.delete_invite(code).await?;
        Ok(())
    }

    /// What an invite leads to, for anyone with a Melior role.
    pub async fn preview_invite(
        &self,
        code: &InviteCode,
        user: &User,
    ) -> Result<InvitePreview, CommunitiesError> {
        require_role(user)?;
        let invite = self.store.invite(code).await?.ok_or(CommunitiesError::NotFound)?;
        let state =
            self.store.load(&invite.community_id).await?.ok_or(CommunitiesError::NotFound)?;
        let member_count = self.store.member_count(state.id()).await?;
        let usable = invite.is_usable(now_ms());
        Ok(InvitePreview { invite, community: state.community, member_count, usable })
    }

    /// Joins the invite's community. A member already in it stays without using
    /// up the invite.
    #[tracing::instrument(name = "communities.redeem", skip_all, fields(user = %user.username))]
    pub async fn redeem_invite(
        &self,
        code: &InviteCode,
        user: &User,
    ) -> Result<CommunityId, CommunitiesError> {
        require_role(user)?;
        let member = Member::new(user.id.clone(), user.username.clone());
        match self.store.redeem_invite(code, &member, now_ms()).await? {
            Redemption::Joined(id) => {
                info!(community = %id, user = %user.username, "member joined");
                self.publish(&id, CommunityEvent::MemberJoined { user_id: user.id.clone() }).await;
                Ok(id)
            }
            Redemption::AlreadyMember(id) => Ok(id),
            Redemption::NotUsable => Err(CommunitiesError::Gone),
            Redemption::Unknown => Err(CommunitiesError::NotFound),
        }
    }
}
