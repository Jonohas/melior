//! Communities on a real Postgres. Needs a database, so it is ignored by default:
//!
//! ```sh
//! MELIOR_TEST_DATABASE_URL=postgres://user:password@127.0.0.1:5432/db \
//!     cargo test -p melior-infrastructure --test postgres_communities -- --ignored
//! ```

use melior_application::{ChatRepository, CommunityRepository, Redemption};
use melior_domain::{
    Category, CategoryId, ChannelKind, ChannelMedia, ChatText, CommunityId, CommunityState, Invite,
    InviteSettings, MediaMode, Member, Name, Override, OverrideTarget, Permissions, Role, RoleId,
    Topic, User, UserId,
};
use melior_infrastructure::postgres::{
    Database, PostgresChatRepository, PostgresCommunityRepository,
};
use std::collections::BTreeSet;
use std::sync::Arc;

fn database() -> Arc<Database> {
    let url = std::env::var("MELIOR_TEST_DATABASE_URL").expect("MELIOR_TEST_DATABASE_URL");
    Arc::new(Database::new(&url).expect("database url"))
}

/// A user no earlier run used, so runs do not disturb each other.
fn user(name: &str) -> User {
    let id = format!("sub-{name}-{}", CommunityId::generate());
    User::new(UserId::new(id), name, None, [User::ROLE_MEMBER.to_owned()])
}

fn name(raw: &str) -> Name {
    Name::parse(raw).unwrap()
}

async fn created(repository: &PostgresCommunityRepository, owner: &User) -> CommunityState {
    let (state, member) = CommunityState::create(CommunityId::generate(), name("Town"), owner);
    repository.create(&state, &member).await.unwrap();
    state
}

fn role(position: u32) -> Role {
    Role {
        id: RoleId::generate(),
        name: name("mod"),
        color: Some(0x00FF_00FF),
        position,
        permissions: Permissions::MANAGE_CHANNELS.union(Permissions::KICK_MEMBERS),
    }
}

fn invite(state: &CommunityState, settings: InviteSettings, now_ms: i64) -> Invite {
    let owner = Member::new(state.community.owner_id.clone(), "owner");
    Invite::new(state.id().clone(), &owner, settings, now_ms)
}

fn member(who: &User) -> Member {
    Member::new(who.id.clone(), who.username.clone())
}

async fn join(repository: &PostgresCommunityRepository, state: &CommunityState, who: &User) {
    let invite = invite(state, InviteSettings::default(), 0);
    repository.save_invite(&invite).await.unwrap();
    let redeemed = repository.redeem_invite(&invite.code, &member(who), 0).await.unwrap();
    assert_eq!(redeemed, Redemption::Joined(state.id().clone()));
}

/// Overrides in a stable order, structure by position.
fn sorted(mut state: CommunityState) -> CommunityState {
    state.channels.sort_by_key(|c| c.position);
    state.categories.sort_by_key(|c| c.position);
    state.roles.sort_by_key(|r| r.position);
    for channel in &mut state.channels {
        channel.overrides.sort_by(|a, b| a.target.cmp(&b.target));
    }
    for category in &mut state.categories {
        category.overrides.sort_by(|a, b| a.target.cmp(&b.target));
    }
    state
}

#[tokio::test]
#[ignore = "needs MELIOR_TEST_DATABASE_URL"]
async fn a_community_is_stored_and_loaded_with_its_whole_structure() {
    let repository = PostgresCommunityRepository::new(database());
    let alice = user("alice");
    let mut state = created(&repository, &alice).await;

    let loaded = repository.load(state.id()).await.unwrap().unwrap();
    assert_eq!(sorted(loaded), sorted(state.clone()));

    let voice = state.channels.iter().position(|c| c.kind == ChannelKind::Voice).unwrap();
    let channel = &mut state.channels[voice];
    channel.overrides = vec![
        Override {
            target: OverrideTarget::Role(RoleId::everyone(&state.community.id)),
            allow: Permissions::NONE,
            deny: Permissions::VIEW_CHANNEL,
        },
        Override {
            target: OverrideTarget::Member(alice.id.clone()),
            allow: Permissions::VIEW_CHANNEL,
            deny: Permissions::NONE,
        },
    ];
    channel.synced = false;
    channel.media = ChannelMedia { mode: MediaMode::PeerToPeer, user_limit: Some(3) };
    channel.topic = Topic::parse("stand-up").unwrap();
    channel.category_id = None;
    let channel = channel.clone();
    repository.save_channel(state.id(), &channel).await.unwrap();

    let by_channel = repository.load_by_channel(&channel.id).await.unwrap().unwrap();
    assert_eq!(sorted(by_channel), sorted(state.clone()));
    assert!(repository.load(&CommunityId::generate()).await.unwrap().is_none());
}

#[tokio::test]
#[ignore = "needs MELIOR_TEST_DATABASE_URL"]
async fn members_are_listed_with_their_roles_and_their_communities() {
    let repository = PostgresCommunityRepository::new(database());
    let (alice, bob) = (user("alice"), user("bob"));
    let state = created(&repository, &alice).await;
    join(&repository, &state, &bob).await;
    let moderator = role(1);
    repository.save_role(state.id(), &moderator).await.unwrap();

    let mut bob_member = member(&bob);
    bob_member.role_ids = BTreeSet::from([moderator.id.clone()]);
    repository.set_member_roles(state.id(), &bob_member).await.unwrap();

    assert_eq!(repository.member(state.id(), &bob.id).await.unwrap(), Some(bob_member.clone()));
    assert_eq!(repository.members(state.id()).await.unwrap().len(), 2);
    assert_eq!(repository.member_count(state.id()).await.unwrap(), 2);
    let listed = repository.list_for(&bob.id).await.unwrap();
    assert_eq!(listed, vec![state.community.clone()]);

    repository.remove_member(state.id(), &bob.id).await.unwrap();
    assert_eq!(repository.member(state.id(), &bob.id).await.unwrap(), None);
    assert!(repository.list_for(&bob.id).await.unwrap().is_empty());
}

#[tokio::test]
#[ignore = "needs MELIOR_TEST_DATABASE_URL"]
async fn deleting_a_role_takes_it_from_members_and_overrides() {
    let repository = PostgresCommunityRepository::new(database());
    let (alice, bob) = (user("alice"), user("bob"));
    let mut state = created(&repository, &alice).await;
    join(&repository, &state, &bob).await;
    let moderator = role(1);
    repository.save_role(state.id(), &moderator).await.unwrap();
    let mut bob_member = member(&bob);
    bob_member.role_ids = BTreeSet::from([moderator.id.clone()]);
    repository.set_member_roles(state.id(), &bob_member).await.unwrap();
    state.categories[0].overrides = vec![Override {
        target: OverrideTarget::Role(moderator.id.clone()),
        allow: Permissions::CONNECT,
        deny: Permissions::NONE,
    }];
    repository.save_category(state.id(), &state.categories[0]).await.unwrap();

    repository.delete_role(state.id(), &moderator.id).await.unwrap();

    let loaded = repository.load(state.id()).await.unwrap().unwrap();
    assert!(loaded.role(&moderator.id).is_none());
    assert!(loaded.categories.iter().all(|c| c.overrides.is_empty()));
    let bob_now = repository.member(state.id(), &bob.id).await.unwrap().unwrap();
    assert!(bob_now.role_ids.is_empty());
}

#[tokio::test]
#[ignore = "needs MELIOR_TEST_DATABASE_URL"]
async fn deleting_a_category_keeps_its_channels_and_deleting_a_channel_its_history() {
    let database = database();
    let repository = PostgresCommunityRepository::new(database.clone());
    let chat = PostgresChatRepository::new(database);
    let state = created(&repository, &user("alice")).await;
    let extra = Category {
        id: CategoryId::generate(),
        name: name("Extra"),
        position: 5,
        overrides: Vec::new(),
    };
    repository.save_category(state.id(), &extra).await.unwrap();
    let text = state.channels.iter().find(|c| c.kind == ChannelKind::Text).unwrap().clone();
    let category = text.category_id.clone().unwrap();

    repository.delete_category(state.id(), &category).await.unwrap();

    let loaded = repository.load(state.id()).await.unwrap().unwrap();
    assert_eq!(loaded.channel(&text.id).unwrap().category_id, None);
    assert_eq!(loaded.categories.len(), 2);

    chat.append(&text.id, "alice", &ChatText::parse("hi").unwrap().into()).await.unwrap();
    repository.delete_channel(&text.id).await.unwrap();
    assert!(chat.page(&text.id, None, 10).await.unwrap().is_empty());
    assert!(repository.load_by_channel(&text.id).await.unwrap().is_none());
}

#[tokio::test]
#[ignore = "needs MELIOR_TEST_DATABASE_URL"]
async fn renaming_and_deleting_a_community() {
    let repository = PostgresCommunityRepository::new(database());
    let mut state = created(&repository, &user("alice")).await;
    state.community.name = name("City");
    repository.rename(&state.community).await.unwrap();
    let loaded = repository.load(state.id()).await.unwrap().unwrap();
    assert_eq!(loaded.community.name.as_str(), "City");

    repository.delete(state.id()).await.unwrap();
    assert!(repository.load(state.id()).await.unwrap().is_none());
    assert!(repository.load_by_channel(&state.channels[0].id).await.unwrap().is_none());
}

#[tokio::test]
#[ignore = "needs MELIOR_TEST_DATABASE_URL"]
async fn two_redeemers_cannot_both_take_the_last_use() {
    let repository = Arc::new(PostgresCommunityRepository::new(database()));
    let state = created(&repository, &user("alice")).await;
    let settings = InviteSettings { max_uses: Some(1), max_age_seconds: None };
    let invite = invite(&state, settings, 0);
    repository.save_invite(&invite).await.unwrap();

    let redeemers: Vec<_> = (0..8)
        .map(|n| {
            let (repository, code) = (repository.clone(), invite.code.clone());
            let who = member(&user(&format!("u{n}")));
            tokio::spawn(async move { repository.redeem_invite(&code, &who, 0).await.unwrap() })
        })
        .collect();
    let mut joined = 0;
    for redeemer in redeemers {
        if matches!(redeemer.await.unwrap(), Redemption::Joined(_)) {
            joined += 1;
        }
    }

    assert_eq!(joined, 1);
    assert_eq!(repository.invite(&invite.code).await.unwrap().unwrap().uses, 1);
    assert_eq!(repository.member_count(state.id()).await.unwrap(), 2);
}

#[tokio::test]
#[ignore = "needs MELIOR_TEST_DATABASE_URL"]
async fn an_expired_invite_admits_nobody_and_a_member_uses_none() {
    let repository = PostgresCommunityRepository::new(database());
    let alice = user("alice");
    let state = created(&repository, &alice).await;
    let settings = InviteSettings { max_age_seconds: Some(1800), max_uses: None };
    let expiring = invite(&state, settings, 0);
    repository.save_invite(&expiring).await.unwrap();

    let late = repository.redeem_invite(&expiring.code, &member(&user("bob")), 1_800_000).await;
    assert_eq!(late.unwrap(), Redemption::NotUsable);
    let owner = repository.redeem_invite(&expiring.code, &member(&alice), 0).await.unwrap();
    assert_eq!(owner, Redemption::AlreadyMember(state.id().clone()));
    assert_eq!(repository.invite(&expiring.code).await.unwrap().unwrap().uses, 0);

    let unknown = melior_domain::InviteCode::generate();
    assert_eq!(
        repository.redeem_invite(&unknown, &member(&alice), 0).await.unwrap(),
        Redemption::Unknown
    );
    assert_eq!(repository.invites(state.id()).await.unwrap(), vec![expiring.clone()]);
    repository.delete_invite(&expiring.code).await.unwrap();
    assert!(repository.invite(&expiring.code).await.unwrap().is_none());
}
