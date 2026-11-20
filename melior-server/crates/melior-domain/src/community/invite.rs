//! Invites: codes that let someone with a Melior role join a community.

use super::{CommunityError, CommunityId, InviteCode, Member, Permissions, Refusal, Standing};
use crate::identity::UserId;

/// How long and how often an invite may be used, as a client asks for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InviteSettings {
    pub max_age_seconds: Option<u64>,
    pub max_uses: Option<u32>,
}

impl InviteSettings {
    /// The lifetimes an invite may have, in seconds; none means forever.
    pub const MAX_AGES: [u64; 6] = [1800, 3600, 21_600, 43_200, 86_400, 604_800];
    pub const MAX_USES_MAX: u32 = 100;

    /// Checks both settings against the allowed choices.
    pub fn check(self) -> Result<Self, CommunityError> {
        if self.max_age_seconds.is_some_and(|age| !Self::MAX_AGES.contains(&age)) {
            return Err(CommunityError::InvalidMaxAge);
        }
        if self.max_uses.is_some_and(|uses| !(1..=Self::MAX_USES_MAX).contains(&uses)) {
            return Err(CommunityError::InvalidMaxUses);
        }
        Ok(self)
    }
}

/// An invite to a community.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invite {
    pub code: InviteCode,
    pub community_id: CommunityId,
    /// The creator's username.
    pub created_by: String,
    pub creator_id: UserId,
    /// Milliseconds since the Unix epoch; none: never expires.
    pub expires_at_ms: Option<i64>,
    pub max_uses: Option<u32>,
    pub uses: u32,
}

impl Invite {
    /// A new, unused invite by `creator`, created at `now_ms`.
    #[must_use]
    pub fn new(
        community_id: CommunityId,
        creator: &Member,
        settings: InviteSettings,
        now_ms: i64,
    ) -> Self {
        let expires_at_ms = settings.max_age_seconds.map(|seconds| {
            let millis = i64::try_from(seconds.saturating_mul(1000)).unwrap_or(i64::MAX);
            now_ms.saturating_add(millis)
        });
        Self {
            code: InviteCode::generate(),
            community_id,
            created_by: creator.username.clone(),
            creator_id: creator.user_id.clone(),
            expires_at_ms,
            max_uses: settings.max_uses,
            uses: 0,
        }
    }

    /// Not expired, and not used up.
    #[must_use]
    pub fn is_usable(&self, now_ms: i64) -> bool {
        let fresh = self.expires_at_ms.is_none_or(|expires| now_ms < expires);
        let left = self.max_uses.is_none_or(|max| self.uses < max);
        fresh && left
    }

    /// Its creator may delete an invite, and so may whoever manages channels.
    pub fn may_delete(&self, actor: &Standing) -> Result<(), Refusal> {
        if *actor.user_id() == self.creator_id {
            return Ok(());
        }
        actor.require(Permissions::MANAGE_CHANNELS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(max_age_seconds: Option<u64>, max_uses: Option<u32>) -> InviteSettings {
        InviteSettings { max_age_seconds, max_uses }
    }

    fn invite(max_age: Option<u64>, max_uses: Option<u32>) -> Invite {
        let creator = Member::new(UserId::new("sub-alice"), "alice");
        Invite::new(CommunityId::generate(), &creator, settings(max_age, max_uses), 1_000)
    }

    #[test]
    fn only_the_listed_lifetimes_and_one_to_a_hundred_uses_are_accepted() {
        for age in InviteSettings::MAX_AGES {
            assert!(settings(Some(age), None).check().is_ok());
        }
        assert!(settings(None, None).check().is_ok());
        assert_eq!(settings(Some(60), None).check(), Err(CommunityError::InvalidMaxAge));
        assert!(settings(None, Some(1)).check().is_ok());
        assert!(settings(None, Some(100)).check().is_ok());
        for bad in [0, 101] {
            assert_eq!(settings(None, Some(bad)).check(), Err(CommunityError::InvalidMaxUses));
        }
    }

    #[test]
    fn an_invite_expires_after_its_lifetime() {
        let invite = invite(Some(1800), None);
        assert_eq!(invite.expires_at_ms, Some(1_000 + 1_800_000));
        assert!(invite.is_usable(1_800_999));
        assert!(!invite.is_usable(1_801_000));
    }

    #[test]
    fn an_invite_without_a_lifetime_never_expires() {
        let invite = invite(None, None);
        assert_eq!(invite.expires_at_ms, None);
        assert!(invite.is_usable(i64::MAX));
    }

    #[test]
    fn an_invite_is_used_up_at_its_maximum() {
        let mut invite = invite(None, Some(2));
        assert!(invite.is_usable(0));
        invite.uses = 1;
        assert!(invite.is_usable(0));
        invite.uses = 2;
        assert!(!invite.is_usable(0));
    }

    #[test]
    fn a_new_invite_is_unused_and_names_its_creator() {
        let invite = invite(None, Some(5));
        assert_eq!(invite.uses, 0);
        assert_eq!(invite.created_by, "alice");
        assert_eq!(invite.creator_id, UserId::new("sub-alice"));
        assert!(InviteCode::parse(invite.code.as_str()).is_some());
    }
}
