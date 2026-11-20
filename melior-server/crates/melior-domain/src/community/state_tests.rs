//! The community aggregate: the seed, what members see, and the changes they make.

use super::{
    CategoryChanges, ChannelChanges, ChannelDraft, ChannelKind, CommunityError, CommunityId,
    CommunityState, InviteSettings, Member, Name, Override, OverrideTarget, Permissions, Refusal,
    Rejected, RoleChanges, RoleDraft, RoleId, Topic,
};
use crate::conference::{ChannelMedia, MediaMode, MeshPolicy};
use crate::identity::{User, UserId};
use std::collections::BTreeSet;

fn user(name: &str) -> User {
    User::new(UserId::new(format!("sub-{name}")), name, None, [User::ROLE_MEMBER.to_owned()])
}

fn name(raw: &str) -> Name {
    Name::parse(raw).unwrap()
}

fn created() -> (CommunityState, Member) {
    CommunityState::create(CommunityId::generate(), name("Melior"), &user("alice"))
}

fn member(id: &str, roles: &[&RoleId]) -> Member {
    let mut member = Member::new(UserId::new(id), id);
    member.role_ids = roles.iter().map(|id| (*id).clone()).collect();
    member
}

fn draft(position_name: &str, permissions: Permissions) -> RoleDraft {
    RoleDraft { name: name(position_name), color: None, permissions }
}

/// A community with a moderator role at position 2 that may manage everything but
/// administration, and a helper role at position 1.
fn staffed() -> (CommunityState, RoleId, RoleId) {
    let (mut state, owner) = created();
    let actor = state.standing(&owner);
    let mut moderator = state
        .new_role(&actor, draft("mod", Permissions::ALL.difference(Permissions::ADMINISTRATOR)))
        .unwrap();
    moderator.position = 2;
    let helper = state.new_role(&actor, draft("helper", Permissions::NONE)).unwrap();
    let mut helper = helper;
    helper.position = 1;
    let ids = (moderator.id.clone(), helper.id.clone());
    state.roles.extend([moderator, helper]);
    (state, ids.0, ids.1)
}

#[test]
fn a_new_community_is_seeded_like_discord() {
    let (state, owner) = created();

    assert_eq!(owner.user_id, UserId::new("sub-alice"));
    assert!(owner.role_ids.is_empty());
    assert_eq!(state.roles.len(), 1);
    assert_eq!(state.roles[0].id, state.everyone_id());
    assert_eq!(state.roles[0].name.as_str(), "@everyone");
    assert_eq!(state.roles[0].position, 0);
    assert_eq!(state.roles[0].permissions, Permissions::EVERYONE_DEFAULT);

    let categories: Vec<&str> = state.categories.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(categories, ["Text channels", "Voice channels"]);
    let channels: Vec<(&str, ChannelKind, Option<_>)> =
        state.channels.iter().map(|c| (c.name.as_str(), c.kind, c.category_id.clone())).collect();
    assert_eq!(
        channels,
        [
            ("general", ChannelKind::Text, Some(state.categories[0].id.clone())),
            ("General", ChannelKind::Voice, Some(state.categories[1].id.clone())),
        ]
    );
    assert!(state.channels.iter().all(|c| c.synced));
}

#[test]
fn members_see_the_channels_they_may_view_in_position_order() {
    let (mut state, _) = created();
    let deny_view = Override {
        target: OverrideTarget::Role(state.everyone_id()),
        allow: Permissions::NONE,
        deny: Permissions::VIEW_CHANNEL,
    };
    state.channels[0].position = 5;
    state.channels[1].synced = false;
    state.channels[1].overrides = vec![deny_view];

    let bob = state.standing(&member("bob", &[]));
    let visible: Vec<&str> =
        state.visible_channels(&bob).iter().map(|(c, _)| c.name.as_str()).collect();
    assert_eq!(visible, ["general"]);

    let owner = state.standing(&Member::new(state.community.owner_id.clone(), "alice"));
    let all: Vec<&str> =
        state.visible_channels(&owner).iter().map(|(c, _)| c.name.as_str()).collect();
    assert_eq!(all, ["General", "general"]);
}

#[test]
fn voice_admission_needs_a_voice_channel_view_and_connect() {
    let (mut state, _) = created();
    let bob = state.standing(&member("bob", &[]));
    let text = state.channels[0].clone();
    let voice = state.channels[1].clone();

    assert_eq!(state.admit_voice(&bob, &text), Err(Refusal::NotVoice));
    assert!(state.admit_voice(&bob, &voice).is_ok());

    state.roles[0].permissions = Permissions::VIEW_CHANNEL;
    let bob = state.standing(&member("bob", &[]));
    assert_eq!(state.admit_voice(&bob, &voice), Err(Refusal::Missing("connect")));
    state.roles[0].permissions = Permissions::NONE;
    let bob = state.standing(&member("bob", &[]));
    assert_eq!(state.admit_voice(&bob, &voice), Err(Refusal::Missing("view_channel")));
}

#[test]
fn renaming_is_for_the_owner_and_administrators() {
    let (state, owner) = created();
    let renamed = state.rename(&state.standing(&owner), name("Other")).unwrap();
    assert_eq!(renamed.name.as_str(), "Other");
    assert!(matches!(
        state.rename(&state.standing(&member("bob", &[])), name("Mine")),
        Err(Rejected::Refused(Refusal::Missing("administrator")))
    ));
}

#[test]
fn the_owner_creates_roles_above_every_existing_one() {
    let (state, moderator, _) = staffed();
    let owner = state.standing(&Member::new(state.community.owner_id.clone(), "alice"));
    let role = state.new_role(&owner, draft("top", Permissions::ALL)).unwrap();
    assert_eq!(role.position, 3);
    assert!(state.role(&moderator).is_some());
}

#[test]
fn a_moderator_creates_roles_just_below_its_own() {
    let (state, moderator, _) = staffed();
    let mo = state.standing(&member("mo", &[&moderator]));
    let role = state.new_role(&mo, draft("new", Permissions::CONNECT)).unwrap();
    assert_eq!(role.position, 1, "shares a position below the moderator rather than reaching it");
}

#[test]
fn a_member_whose_top_role_is_at_one_has_no_room_below_it() {
    let (mut state, _, helper) = staffed();
    let index = state.roles.iter().position(|r| r.id == helper).unwrap();
    state.roles[index].permissions = Permissions::MANAGE_ROLES;
    let actor = state.standing(&member("h", &[&helper]));
    assert_eq!(
        state.new_role(&actor, draft("x", Permissions::NONE)),
        Err(Rejected::Refused(Refusal::RoleNotBelow))
    );
}

#[test]
fn role_input_is_checked_before_permissions() {
    let (state, owner) = created();
    let actor = state.standing(&owner);
    let bad_color = RoleDraft { color: Some(0x100_0000), ..draft("x", Permissions::NONE) };
    assert_eq!(
        state.new_role(&actor, bad_color),
        Err(Rejected::Invalid(CommunityError::InvalidColor))
    );
}

#[test]
fn everyone_keeps_its_name_colour_and_position_but_its_permissions_may_change() {
    let (state, owner) = created();
    let actor = state.standing(&owner);
    let everyone = state.everyone_id();
    for changes in [
        RoleChanges { name: Some(name("all")), ..RoleChanges::default() },
        RoleChanges { color: Some(Some(1)), ..RoleChanges::default() },
        RoleChanges { position: Some(4), ..RoleChanges::default() },
    ] {
        assert_eq!(
            state.edit_role(&actor, &everyone, changes),
            Err(Rejected::Invalid(CommunityError::EveryoneRole))
        );
    }
    let edited = state
        .edit_role(
            &actor,
            &everyone,
            RoleChanges { permissions: Some(Permissions::VIEW_CHANNEL), ..RoleChanges::default() },
        )
        .unwrap();
    assert_eq!(edited.permissions, Permissions::VIEW_CHANNEL);
    assert_eq!(
        state.delete_role(&actor, &everyone),
        Err(Rejected::Invalid(CommunityError::EveryoneRole))
    );
}

#[test]
fn a_role_cannot_move_to_position_zero() {
    let (state, _, helper) = staffed();
    let owner = state.standing(&Member::new(state.community.owner_id.clone(), "alice"));
    assert_eq!(
        state.edit_role(
            &owner,
            &helper,
            RoleChanges { position: Some(0), ..RoleChanges::default() }
        ),
        Err(Rejected::Invalid(CommunityError::InvalidPosition))
    );
}

#[test]
fn editing_checks_only_permissions_the_role_did_not_have() {
    let (mut state, moderator, helper) = staffed();
    let index = state.roles.iter().position(|r| r.id == helper).unwrap();
    state.roles[index].permissions = Permissions::ADMINISTRATOR;
    let mo = state.standing(&member("mo", &[&moderator]));

    let renamed = state.edit_role(
        &mo,
        &helper,
        RoleChanges { name: Some(name("aide")), ..RoleChanges::default() },
    );
    assert!(renamed.is_ok(), "keeping a permission the actor lacks is not granting it");

    let edited = state.edit_role(
        &mo,
        &moderator,
        RoleChanges { name: Some(name("x")), ..RoleChanges::default() },
    );
    assert_eq!(edited, Err(Rejected::Refused(Refusal::RoleNotBelow)));
}

#[test]
fn assigning_roles_checks_every_role_added_or_removed() {
    let (state, moderator, helper) = staffed();
    let mo = state.standing(&member("mo", &[&moderator]));
    let bob = member("bob", &[]);

    let with_helper = state.assign_roles(&mo, &bob, BTreeSet::from([helper.clone()])).unwrap();
    assert_eq!(with_helper.role_ids, BTreeSet::from([helper.clone()]));
    assert!(state.assign_roles(&mo, &with_helper, BTreeSet::new()).is_ok());

    assert_eq!(
        state.assign_roles(&mo, &bob, BTreeSet::from([moderator.clone()])),
        Err(Rejected::Refused(Refusal::RoleNotBelow))
    );
    assert_eq!(
        state.assign_roles(&mo, &bob, BTreeSet::from([state.everyone_id()])),
        Err(Rejected::Invalid(CommunityError::EveryoneRole))
    );
    let unknown = RoleId::generate();
    assert_eq!(
        state.assign_roles(&mo, &bob, BTreeSet::from([unknown.clone()])),
        Err(Rejected::Invalid(CommunityError::UnknownRole(unknown.to_string())))
    );
}

#[test]
fn a_moderator_kicks_members_below_it() {
    let (state, moderator, helper) = staffed();
    let mo = state.standing(&member("mo", &[&moderator]));
    assert_eq!(state.kick(&mo, &member("bob", &[&helper])), Ok(()));
    assert_eq!(
        state.kick(&mo, &Member::new(state.community.owner_id.clone(), "alice")),
        Err(Rejected::Refused(Refusal::CannotKickOwner))
    );
    assert_eq!(CommunityState::leave(&mo), Ok(()));
}

#[test]
fn channels_need_manage_channels_and_valid_media() {
    let (state, owner) = created();
    let owner = state.standing(&owner);
    let bob = state.standing(&member("bob", &[]));
    let category = state.categories[1].id.clone();
    let voice = ChannelDraft {
        name: name("Stand-up"),
        kind: ChannelKind::Voice,
        topic: Topic::default(),
        category_id: Some(category.clone()),
        media: ChannelMedia { mode: MediaMode::PeerToPeer, user_limit: Some(3) },
    };

    assert_eq!(
        state.new_channel(&bob, voice.clone(), MeshPolicy::new(4)),
        Err(Rejected::Refused(Refusal::Missing("manage_channels")))
    );
    let channel = state.new_channel(&owner, voice.clone(), MeshPolicy::new(4)).unwrap();
    assert!(channel.synced, "a channel in a category starts synced");
    assert_eq!(channel.position, 2);

    let too_big = ChannelDraft {
        media: ChannelMedia { mode: MediaMode::PeerToPeer, user_limit: Some(5) },
        ..voice.clone()
    };
    assert_eq!(
        state.new_channel(&owner, too_big, MeshPolicy::new(4)),
        Err(Rejected::Invalid(CommunityError::PeerToPeerLimit(4)))
    );
    let text_with_media = ChannelDraft { kind: ChannelKind::Text, ..voice };
    assert_eq!(
        state.new_channel(&owner, text_with_media, MeshPolicy::new(4)),
        Err(Rejected::Invalid(CommunityError::TextChannelMedia))
    );
}

#[test]
fn editing_overrides_unsyncs_a_channel_unless_synced_is_set_again() {
    let (state, owner) = created();
    let owner = state.standing(&owner);
    let id = state.channels[1].id.clone();
    let overrides = vec![Override {
        target: OverrideTarget::Member(UserId::new("sub-bob")),
        allow: Permissions::CONNECT,
        deny: Permissions::NONE,
    }];

    let edited = state
        .edit_channel(
            &owner,
            &id,
            ChannelChanges { overrides: Some(overrides.clone()), ..ChannelChanges::default() },
            MeshPolicy::OFF,
        )
        .unwrap();
    assert!(!edited.synced);
    assert_eq!(edited.overrides, overrides);

    let resynced = state
        .edit_channel(
            &owner,
            &id,
            ChannelChanges {
                overrides: Some(overrides),
                synced: Some(true),
                ..ChannelChanges::default()
            },
            MeshPolicy::OFF,
        )
        .unwrap();
    assert!(resynced.synced);

    let moved = state
        .edit_channel(
            &owner,
            &id,
            ChannelChanges { category_id: Some(None), ..ChannelChanges::default() },
            MeshPolicy::OFF,
        )
        .unwrap();
    assert_eq!(moved.category_id, None);
}

#[test]
fn overrides_must_name_known_roles_once_each() {
    let (state, owner) = created();
    let owner = state.standing(&owner);
    let id = state.categories[0].id.clone();
    let unknown = RoleId::generate();
    let entry = |target| Override { target, allow: Permissions::NONE, deny: Permissions::NONE };

    let duplicate = vec![
        entry(OverrideTarget::Role(state.everyone_id())),
        entry(OverrideTarget::Role(state.everyone_id())),
    ];
    assert_eq!(
        state.edit_category(
            &owner,
            &id,
            CategoryChanges { overrides: Some(duplicate), ..CategoryChanges::default() }
        ),
        Err(Rejected::Invalid(CommunityError::InvalidOverride))
    );
    let foreign = vec![entry(OverrideTarget::Role(unknown.clone()))];
    assert_eq!(
        state.edit_category(
            &owner,
            &id,
            CategoryChanges { overrides: Some(foreign), ..CategoryChanges::default() }
        ),
        Err(Rejected::Invalid(CommunityError::UnknownRole(unknown.to_string())))
    );
}

#[test]
fn categories_need_manage_channels() {
    let (state, owner) = created();
    let bob = state.standing(&member("bob", &[]));
    assert!(state.new_category(&bob, name("x")).is_err());
    let category = state.new_category(&state.standing(&owner), name("x")).unwrap();
    assert_eq!(category.position, 2);
    assert!(state.delete_category(&bob, &state.categories[0].id).is_err());
}

#[test]
fn invites_need_create_invite_and_valid_settings() {
    let (mut state, owner) = created();
    let bob_member = member("bob", &[]);
    let bob = state.standing(&bob_member);
    let invite = state.new_invite(&bob, &bob_member, InviteSettings::default(), 0).unwrap();
    assert_eq!(invite.created_by, "bob");
    assert_eq!(invite.may_delete(&bob), Ok(()));
    let carol = state.standing(&member("carol", &[]));
    assert_eq!(invite.may_delete(&carol), Err(Refusal::Missing("manage_channels")));
    assert_eq!(invite.may_delete(&state.standing(&owner)), Ok(()));

    let bad = InviteSettings { max_uses: Some(0), max_age_seconds: None };
    assert_eq!(
        state.new_invite(&bob, &bob_member, bad, 0),
        Err(Rejected::Invalid(CommunityError::InvalidMaxUses))
    );

    state.roles[0].permissions = Permissions::VIEW_CHANNEL;
    let bob = state.standing(&bob_member);
    assert!(state.new_invite(&bob, &bob_member, InviteSettings::default(), 0).is_err());
    assert!(CommunityState::may_list_invites(&bob).is_err());
}
