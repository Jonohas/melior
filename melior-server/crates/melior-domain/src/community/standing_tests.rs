//! The permission resolution, line by line, and the hierarchy rules.

use super::{
    Category, CategoryId, Channel, ChannelKind, Community, CommunityId, Member, Name, Override,
    OverrideTarget, Permissions, Rank, Refusal, Role, RoleId, Standing, Topic,
};
use crate::conference::{ChannelId, ChannelMedia};
use crate::identity::UserId;

const VIEW: Permissions = Permissions::VIEW_CHANNEL;
const CONNECT: Permissions = Permissions::CONNECT;
const SEND: Permissions = Permissions::SEND_MESSAGES;

struct World {
    community: Community,
    roles: Vec<Role>,
    /// Position 1, no permissions of its own.
    helper: RoleId,
    /// Position 2, may manage roles, channels and kick.
    moderator: RoleId,
    /// Position 3, administrator.
    admin: RoleId,
}

fn role(name: &str, position: u32, permissions: Permissions) -> Role {
    Role {
        id: RoleId::generate(),
        name: Name::parse(name).unwrap(),
        color: None,
        position,
        permissions,
    }
}

fn world(everyone: Permissions) -> World {
    let id = CommunityId::generate();
    let everyone = Role { id: RoleId::everyone(&id), ..role("@everyone", 0, everyone) };
    let helper = role("helper", 1, Permissions::NONE);
    let manage = Permissions::MANAGE_ROLES
        .union(Permissions::MANAGE_CHANNELS)
        .union(Permissions::KICK_MEMBERS)
        .union(Permissions::EVERYONE_DEFAULT);
    let moderator = role("moderator", 2, manage);
    let admin = role("admin", 3, Permissions::ADMINISTRATOR);
    World {
        community: Community {
            id,
            name: Name::parse("Melior").unwrap(),
            owner_id: UserId::new("owner"),
            owner_username: "owner".into(),
        },
        helper: helper.id.clone(),
        moderator: moderator.id.clone(),
        admin: admin.id.clone(),
        roles: vec![everyone, helper, moderator, admin],
    }
}

fn member(id: &str, roles: &[&RoleId]) -> Member {
    let mut member = Member::new(UserId::new(id), id);
    member.role_ids = roles.iter().map(|id| (*id).clone()).collect();
    member
}

impl World {
    fn standing(&self, id: &str, roles: &[&RoleId]) -> Standing {
        Standing::new(&self.community, &self.roles, &member(id, roles))
    }

    fn everyone(&self) -> OverrideTarget {
        OverrideTarget::Role(RoleId::everyone(&self.community.id))
    }
}

fn channel(overrides: Vec<Override>) -> Channel {
    Channel {
        id: ChannelId::generate(),
        name: Name::parse("room").unwrap(),
        topic: Topic::default(),
        kind: ChannelKind::Voice,
        category_id: None,
        position: 0,
        synced: false,
        overrides,
        media: ChannelMedia::default(),
    }
}

fn category(overrides: Vec<Override>) -> Category {
    Category { id: CategoryId::generate(), name: Name::parse("c").unwrap(), position: 0, overrides }
}

fn rule(target: OverrideTarget, allow: Permissions, deny: Permissions) -> Override {
    Override { target, allow, deny }
}

fn member_target(id: &str) -> OverrideTarget {
    OverrideTarget::Member(UserId::new(id))
}

#[test]
fn step_1_the_owner_has_everything_everywhere() {
    let w = world(Permissions::NONE);
    let owner = w.standing("owner", &[]);
    let locked = channel(vec![rule(member_target("owner"), Permissions::NONE, Permissions::ALL)]);

    assert!(owner.is_owner());
    assert_eq!(owner.base(), Permissions::ALL);
    assert_eq!(owner.in_channel(&locked, None), Permissions::ALL);
    assert_eq!(owner.rank(), Rank::Owner);
}

#[test]
fn step_2_base_is_everyone_plus_every_role_of_the_member() {
    let w = world(VIEW);
    assert_eq!(w.standing("bob", &[]).base(), VIEW);
    let moderator = w.standing("bob", &[&w.moderator]);
    assert!(moderator.base().contains(Permissions::MANAGE_ROLES));
    assert!(moderator.base().contains(VIEW));
    assert!(!moderator.base().contains(Permissions::ADMINISTRATOR));
}

#[test]
fn step_2_a_role_the_member_does_not_hold_grants_nothing() {
    let w = world(VIEW);
    assert!(!w.standing("bob", &[&w.helper]).base().contains(Permissions::MANAGE_ROLES));
}

#[test]
fn step_3_an_administrator_has_everything_whatever_the_overrides() {
    let w = world(Permissions::NONE);
    let admin = w.standing("ann", &[&w.admin]);
    let locked = channel(vec![rule(w.everyone(), Permissions::NONE, Permissions::ALL)]);

    assert_eq!(admin.base(), Permissions::ALL);
    assert_eq!(admin.in_channel(&locked, None), Permissions::ALL);
}

#[test]
fn step_4_a_synced_channel_in_a_category_uses_the_category_overrides() {
    let w = world(Permissions::EVERYONE_DEFAULT);
    let bob = w.standing("bob", &[]);
    let private = category(vec![rule(w.everyone(), Permissions::NONE, VIEW)]);
    let mut synced = channel(vec![]);
    synced.category_id = Some(private.id.clone());
    synced.synced = true;

    assert_eq!(bob.in_channel(&synced, Some(&private)), Permissions::NONE);
}

#[test]
fn step_4_an_unsynced_channel_uses_its_own_overrides() {
    let w = world(Permissions::EVERYONE_DEFAULT);
    let bob = w.standing("bob", &[]);
    let private = category(vec![rule(w.everyone(), Permissions::NONE, VIEW)]);
    let mut own = channel(vec![rule(w.everyone(), Permissions::NONE, SEND)]);
    own.category_id = Some(private.id.clone());
    own.synced = false;

    let permissions = bob.in_channel(&own, Some(&private));
    assert!(permissions.contains(VIEW));
    assert!(!permissions.contains(SEND));
}

#[test]
fn step_4_a_synced_channel_without_a_category_uses_its_own_overrides() {
    let w = world(Permissions::EVERYONE_DEFAULT);
    let bob = w.standing("bob", &[]);
    let mut loose = channel(vec![rule(w.everyone(), Permissions::NONE, CONNECT)]);
    loose.synced = true;

    assert!(!bob.in_channel(&loose, None).contains(CONNECT));
    assert!(bob.in_channel(&loose, None).contains(VIEW));
}

#[test]
fn step_5_everyone_override_denies_then_allows() {
    let w = world(VIEW.union(CONNECT));
    let bob = w.standing("bob", &[]);
    let swap = channel(vec![rule(w.everyone(), SEND, CONNECT)]);

    let permissions = bob.in_channel(&swap, None);
    assert!(!permissions.contains(CONNECT));
    assert!(permissions.contains(SEND));
}

#[test]
fn step_5_role_overrides_come_after_everyone_and_are_unioned() {
    let w = world(Permissions::EVERYONE_DEFAULT);
    let bob = w.standing("bob", &[&w.helper, &w.moderator]);
    let overrides = vec![
        rule(w.everyone(), Permissions::NONE, CONNECT.union(SEND)),
        rule(OverrideTarget::Role(w.helper.clone()), CONNECT, Permissions::NONE),
        rule(
            OverrideTarget::Role(w.moderator.clone()),
            Permissions::NONE,
            Permissions::READ_HISTORY,
        ),
    ];

    let permissions = bob.in_channel(&channel(overrides), None);
    assert!(permissions.contains(CONNECT), "a role allow beats an @everyone deny");
    assert!(!permissions.contains(SEND));
    assert!(!permissions.contains(Permissions::READ_HISTORY));
}

#[test]
fn step_5_an_allow_of_one_role_beats_a_deny_of_another() {
    let w = world(Permissions::EVERYONE_DEFAULT);
    let bob = w.standing("bob", &[&w.helper, &w.moderator]);
    let overrides = vec![
        rule(OverrideTarget::Role(w.helper.clone()), Permissions::NONE, SEND),
        rule(OverrideTarget::Role(w.moderator.clone()), SEND, Permissions::NONE),
    ];
    assert!(bob.in_channel(&channel(overrides), None).contains(SEND));
}

#[test]
fn step_5_overrides_of_roles_the_member_lacks_do_not_apply() {
    let w = world(Permissions::EVERYONE_DEFAULT);
    let bob = w.standing("bob", &[]);
    let overrides = vec![rule(OverrideTarget::Role(w.helper.clone()), Permissions::NONE, CONNECT)];
    assert!(bob.in_channel(&channel(overrides), None).contains(CONNECT));
}

#[test]
fn step_5_the_member_override_comes_last() {
    let w = world(Permissions::EVERYONE_DEFAULT);
    let bob = w.standing("bob", &[&w.helper]);
    let overrides = vec![
        rule(OverrideTarget::Role(w.helper.clone()), CONNECT, Permissions::NONE),
        rule(member_target("bob"), SEND, CONNECT),
        rule(member_target("carol"), Permissions::NONE, VIEW),
    ];

    let permissions = bob.in_channel(&channel(overrides), None);
    assert!(!permissions.contains(CONNECT), "the member's deny beats a role allow");
    assert!(permissions.contains(SEND));
    assert!(permissions.contains(VIEW), "another member's override does not apply");
}

#[test]
fn step_6_without_view_channel_nothing_is_left() {
    let w = world(Permissions::EVERYONE_DEFAULT);
    let bob = w.standing("bob", &[]);
    let hidden = channel(vec![rule(w.everyone(), CONNECT, VIEW)]);
    assert_eq!(bob.in_channel(&hidden, None), Permissions::NONE);
}

#[test]
fn step_6_a_member_allow_can_reopen_a_hidden_channel() {
    let w = world(Permissions::EVERYONE_DEFAULT);
    let overrides = vec![
        rule(w.everyone(), Permissions::NONE, VIEW),
        rule(member_target("bob"), VIEW, Permissions::NONE),
    ];
    assert!(w.standing("bob", &[]).in_channel(&channel(overrides.clone()), None).contains(VIEW));
    assert_eq!(w.standing("carol", &[]).in_channel(&channel(overrides), None), Permissions::NONE);
}

#[test]
fn community_wide_checks_use_base_permissions_not_overrides() {
    let w = world(Permissions::EVERYONE_DEFAULT);
    let bob = w.standing("bob", &[]);
    assert_eq!(bob.require(Permissions::CREATE_INVITE), Ok(()));
    assert_eq!(bob.require(Permissions::MANAGE_CHANNELS), Err(Refusal::Missing("manage_channels")));
}

#[test]
fn rank_is_the_highest_role_position_and_zero_without_roles() {
    let w = world(Permissions::NONE);
    assert_eq!(w.standing("bob", &[]).rank(), Rank::Position(0));
    assert_eq!(w.standing("bob", &[&w.helper, &w.moderator]).rank(), Rank::Position(2));
    assert!(Rank::Owner > Rank::Position(u32::MAX));
    assert_eq!(Rank::Owner.value(), u32::MAX);
}

#[test]
fn renaming_needs_administrator_and_deleting_needs_the_owner() {
    let w = world(Permissions::EVERYONE_DEFAULT);
    assert_eq!(w.standing("owner", &[]).may_rename_community(), Ok(()));
    assert_eq!(w.standing("ann", &[&w.admin]).may_rename_community(), Ok(()));
    assert!(w.standing("mo", &[&w.moderator]).may_rename_community().is_err());
    assert_eq!(w.standing("owner", &[]).may_delete_community(), Ok(()));
    assert_eq!(w.standing("ann", &[&w.admin]).may_delete_community(), Err(Refusal::OwnerOnly));
}

#[test]
fn changing_a_role_needs_manage_roles() {
    let w = world(Permissions::EVERYONE_DEFAULT);
    assert_eq!(
        w.standing("bob", &[&w.helper]).may_change_role(None, 0, Permissions::NONE),
        Err(Refusal::Missing("manage_roles"))
    );
}

#[test]
fn changing_a_role_needs_its_old_and_new_position_below_the_actor() {
    let w = world(Permissions::EVERYONE_DEFAULT);
    let mo = w.standing("mo", &[&w.moderator]);
    assert_eq!(mo.may_change_role(Some(1), 1, Permissions::NONE), Ok(()));
    assert_eq!(mo.may_change_role(Some(2), 1, Permissions::NONE), Err(Refusal::RoleNotBelow));
    assert_eq!(mo.may_change_role(Some(1), 2, Permissions::NONE), Err(Refusal::RoleNotBelow));
    assert_eq!(mo.may_change_role(None, 3, Permissions::NONE), Err(Refusal::RoleNotBelow));
}

#[test]
fn a_role_cannot_be_granted_more_than_the_actor_has_unless_owner_or_admin() {
    let w = world(Permissions::EVERYONE_DEFAULT);
    let mo = w.standing("mo", &[&w.moderator]);
    assert_eq!(mo.may_change_role(None, 1, Permissions::KICK_MEMBERS), Ok(()));
    assert_eq!(
        mo.may_change_role(None, 1, Permissions::ADMINISTRATOR),
        Err(Refusal::ExceedsOwnPermissions)
    );
    let ann = w.standing("ann", &[&w.admin]);
    assert_eq!(ann.may_change_role(None, 1, Permissions::ADMINISTRATOR), Ok(()));
    let owner = w.standing("owner", &[]);
    assert_eq!(owner.may_change_role(Some(3), 9, Permissions::ALL), Ok(()));
}

#[test]
fn assigning_a_role_needs_manage_roles_and_the_role_below_the_actor() {
    let w = world(Permissions::EVERYONE_DEFAULT);
    let mo = w.standing("mo", &[&w.moderator]);
    assert_eq!(mo.may_assign(1), Ok(()));
    assert_eq!(mo.may_assign(2), Err(Refusal::RoleNotBelow));
    assert_eq!(w.standing("bob", &[]).may_assign(1), Err(Refusal::Missing("manage_roles")));
}

#[test]
fn kicking_needs_kick_members_never_the_owner_and_only_lower_ranks() {
    let w = world(Permissions::EVERYONE_DEFAULT);
    let mo = w.standing("mo", &[&w.moderator]);
    assert_eq!(mo.may_kick(&w.standing("bob", &[&w.helper])), Ok(()));
    assert_eq!(mo.may_kick(&w.standing("other", &[&w.moderator])), Err(Refusal::TargetOutranks));
    assert_eq!(mo.may_kick(&w.standing("ann", &[&w.admin])), Err(Refusal::TargetOutranks));
    assert_eq!(
        w.standing("ann", &[&w.admin]).may_kick(&w.standing("owner", &[])),
        Err(Refusal::CannotKickOwner)
    );
    assert_eq!(
        w.standing("bob", &[]).may_kick(&w.standing("carol", &[])),
        Err(Refusal::Missing("kick_members"))
    );
    assert_eq!(w.standing("owner", &[]).may_kick(&w.standing("ann", &[&w.admin])), Ok(()));
}

#[test]
fn anyone_but_the_owner_may_leave() {
    let w = world(Permissions::EVERYONE_DEFAULT);
    assert_eq!(w.standing("bob", &[]).may_leave(), Ok(()));
    assert_eq!(w.standing("owner", &[]).may_leave(), Err(Refusal::OwnerCannotLeave));
}
