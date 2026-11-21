//! The community use cases against the in-memory ports.

use super::*;
use crate::conference::VoiceEntry;
use melior_domain::{
    ChannelChanges, ChannelDraft, ChannelId, ChannelKind, ChannelMedia, CommunityId,
    InviteSettings, MediaMode, MeshPolicy, Name, Override, OverrideTarget, Permissions, Refusal,
    RoleChanges, RoleDraft, SessionId, Topic, User, UserId, VoiceState,
};
use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

fn user(name: &str) -> User {
    User::new(UserId::new(format!("sub-{name}")), name, None, [User::ROLE_MEMBER.to_owned()])
}

struct Harness {
    service: CommunityService,
    events: Arc<InMemoryCommunityEvents>,
    roster: Arc<InMemoryVoiceRoster>,
}

impl Harness {
    fn new() -> Self {
        let events = Arc::new(InMemoryCommunityEvents::default());
        let roster = Arc::new(InMemoryVoiceRoster::default());
        let service = CommunityService::new(
            Arc::new(InMemoryCommunityRepository::default()),
            events.clone(),
            roster.clone(),
            MeshPolicy::new(4),
        );
        Self { service, events, roster }
    }

    /// alice's community, which every name in `members` joined by invite.
    async fn community(&self, members: &[&str]) -> CommunityView {
        let view = self.service.create(&user("alice"), "Town").await.unwrap();
        let invite = self
            .service
            .create_invite(&view.community.id, &user("alice"), InviteSettings::default())
            .await
            .unwrap();
        for name in members {
            self.service.redeem_invite(&invite.code, &user(name)).await.unwrap();
        }
        view
    }
}

fn channel_of(view: &CommunityView, kind: ChannelKind) -> ChannelId {
    view.channels.iter().find(|c| c.channel.kind == kind).unwrap().channel.id.clone()
}

fn voice_draft(mode: MediaMode, user_limit: Option<u8>) -> ChannelDraft {
    ChannelDraft {
        name: Name::parse("Room").unwrap(),
        kind: ChannelKind::Voice,
        topic: Topic::default(),
        category_id: None,
        media: ChannelMedia { mode, user_limit },
    }
}

#[tokio::test]
async fn the_creator_owns_a_seeded_community_and_sees_it_listed() {
    let h = Harness::new();
    let view = h.community(&[]).await;

    assert!(view.me.is_owner);
    assert_eq!(view.me.permissions, Permissions::ALL);
    let names: Vec<&str> = view.channels.iter().map(|c| c.channel.name.as_str()).collect();
    assert_eq!(names, ["general", "General"]);
    let listed = h.service.list(&user("alice")).await.unwrap();
    assert_eq!(listed.len(), 1);
    assert!(h.service.list(&user("bob")).await.unwrap().is_empty());
}

#[tokio::test]
async fn outsiders_and_guests_are_refused() {
    let h = Harness::new();
    let view = h.community(&[]).await;
    let id = &view.community.id;
    let guest = User::new(UserId::new("sub-guest"), "guest", None, []);

    assert!(matches!(h.service.view(id, &user("bob")).await, Err(CommunitiesError::Forbidden)));
    assert!(matches!(h.service.create(&guest, "x").await, Err(CommunitiesError::Forbidden)));
    assert!(matches!(h.service.list(&guest).await, Err(CommunitiesError::Forbidden)));
    assert!(matches!(
        h.service.view(&CommunityId::generate(), &user("alice")).await,
        Err(CommunitiesError::NotFound)
    ));
    assert!(matches!(
        h.service.create(&user("bob"), "  ").await,
        Err(CommunitiesError::Invalid(_))
    ));
}

#[tokio::test]
async fn renaming_and_deleting_are_published() {
    let h = Harness::new();
    let view = h.community(&["bob"]).await;
    let id = &view.community.id;
    let mut feed = h.events.subscribe(id);

    assert!(matches!(
        h.service.rename(id, &user("bob"), "Mine").await,
        Err(CommunitiesError::Refused(Refusal::Missing("administrator")))
    ));
    let renamed = h.service.rename(id, &user("alice"), "City").await.unwrap();
    assert_eq!(renamed.community.name.as_str(), "City");
    assert_eq!(feed.recv().await.unwrap(), CommunityEvent::StructureChanged);

    assert!(matches!(
        h.service.delete(id, &user("bob")).await,
        Err(CommunitiesError::Refused(Refusal::OwnerOnly))
    ));
    h.service.delete(id, &user("alice")).await.unwrap();
    assert_eq!(feed.recv().await.unwrap(), CommunityEvent::CommunityDeleted);
    assert!(matches!(h.service.view(id, &user("alice")).await, Err(CommunitiesError::NotFound)));
}

#[tokio::test]
async fn invites_admit_members_once_and_run_out() {
    let h = Harness::new();
    let view = h.community(&[]).await;
    let id = &view.community.id;
    let settings = InviteSettings { max_uses: Some(1), max_age_seconds: None };
    let invite = h.service.create_invite(id, &user("alice"), settings).await.unwrap();

    let preview = h.service.preview_invite(&invite.code, &user("bob")).await.unwrap();
    assert_eq!((preview.member_count, preview.usable), (1, true));

    assert_eq!(h.service.redeem_invite(&invite.code, &user("bob")).await.unwrap(), *id);
    assert_eq!(
        h.service.redeem_invite(&invite.code, &user("bob")).await.unwrap(),
        *id,
        "a member gets the id again without using the invite"
    );
    assert!(matches!(
        h.service.redeem_invite(&invite.code, &user("carol")).await,
        Err(CommunitiesError::Gone)
    ));
    let preview = h.service.preview_invite(&invite.code, &user("carol")).await.unwrap();
    assert_eq!((preview.member_count, preview.usable), (2, false));
}

#[tokio::test]
async fn joining_through_an_invite_is_announced_once() {
    let h = Harness::new();
    let view = h.community(&[]).await;
    let id = &view.community.id;
    let invite =
        h.service.create_invite(id, &user("alice"), InviteSettings::default()).await.unwrap();
    let mut feed = h.events.subscribe(id);

    h.service.redeem_invite(&invite.code, &user("bob")).await.unwrap();
    h.service.redeem_invite(&invite.code, &user("bob")).await.unwrap();

    assert_eq!(
        feed.recv().await.unwrap(),
        CommunityEvent::MemberJoined { user_id: user("bob").id }
    );
    assert!(feed.try_recv().is_err(), "a member who was already in is not announced again");
}

#[tokio::test]
async fn invites_are_listed_by_managers_and_deleted_by_creators_or_managers() {
    let h = Harness::new();
    let view = h.community(&["bob", "carol"]).await;
    let id = &view.community.id;
    let invite =
        h.service.create_invite(id, &user("bob"), InviteSettings::default()).await.unwrap();

    assert!(h.service.invites(id, &user("bob")).await.is_err());
    assert_eq!(h.service.invites(id, &user("alice")).await.unwrap().len(), 2);
    assert!(h.service.delete_invite(&invite.code, &user("carol")).await.is_err());
    h.service.delete_invite(&invite.code, &user("bob")).await.unwrap();
    assert!(matches!(
        h.service.preview_invite(&invite.code, &user("bob")).await,
        Err(CommunitiesError::NotFound)
    ));
}

#[tokio::test]
async fn kicking_and_leaving_remove_the_member_and_tell_the_feed() {
    let h = Harness::new();
    let view = h.community(&["bob", "carol"]).await;
    let id = &view.community.id;
    let mut feed = h.events.subscribe(id);

    assert!(h.service.remove_member(id, &user("bob"), &user("carol").id).await.is_err());
    h.service.remove_member(id, &user("alice"), &user("carol").id).await.unwrap();
    h.service.remove_member(id, &user("bob"), &user("bob").id).await.unwrap();
    assert!(matches!(
        h.service.remove_member(id, &user("alice"), &user("alice").id).await,
        Err(CommunitiesError::Refused(Refusal::OwnerCannotLeave))
    ));

    let removed =
        |name: &str, reason| CommunityEvent::MemberRemoved { user_id: user(name).id, reason };
    assert_eq!(feed.recv().await.unwrap(), removed("carol", RemovalReason::Kicked));
    assert_eq!(feed.recv().await.unwrap(), removed("bob", RemovalReason::Left));
    assert_eq!(h.service.members(id, &user("alice")).await.unwrap().len(), 1);
}

#[tokio::test]
async fn roles_grant_what_they_hold_to_the_members_who_hold_them() {
    let h = Harness::new();
    let view = h.community(&["bob", "carol"]).await;
    let id = &view.community.id;
    let draft = RoleDraft {
        name: Name::parse("mod").unwrap(),
        color: Some(0xFF_0000),
        permissions: Permissions::MANAGE_CHANNELS,
    };
    let role = h.service.create_role(id, &user("alice"), draft).await.unwrap();
    assert_eq!(role.position, 1);

    let roles = BTreeSet::from([role.id.clone()]);
    assert!(
        h.service
            .set_member_roles(id, &user("bob"), &user("carol").id, roles.clone())
            .await
            .is_err()
    );
    h.service.set_member_roles(id, &user("alice"), &user("bob").id, roles).await.unwrap();
    let bob = h.service.view(id, &user("bob")).await.unwrap();
    assert!(bob.me.permissions.contains(Permissions::MANAGE_CHANNELS));

    let changes = RoleChanges { permissions: Some(Permissions::NONE), ..RoleChanges::default() };
    h.service.edit_role(id, &user("alice"), &role.id, changes).await.unwrap();
    assert!(
        !h.service
            .view(id, &user("bob"))
            .await
            .unwrap()
            .me
            .permissions
            .contains(Permissions::MANAGE_CHANNELS)
    );

    h.service.delete_role(id, &user("alice"), &role.id).await.unwrap();
    assert!(h.service.view(id, &user("bob")).await.unwrap().me.member.role_ids.is_empty());
    assert!(matches!(
        h.service.delete_role(id, &user("alice"), &role.id).await,
        Err(CommunitiesError::NotFound)
    ));
}

#[tokio::test]
async fn a_hidden_channel_is_left_out_of_the_view_and_refused_at_the_door() {
    let h = Harness::new();
    let view = h.community(&["bob"]).await;
    let id = &view.community.id;
    let voice = channel_of(&view, ChannelKind::Voice);
    assert!(h.service.admit_voice(&voice, &user("bob")).await.is_ok());

    let hide = vec![Override {
        target: OverrideTarget::Role(melior_domain::RoleId::everyone(id)),
        allow: Permissions::NONE,
        deny: Permissions::VIEW_CHANNEL,
    }];
    let changes = ChannelChanges { overrides: Some(hide), ..ChannelChanges::default() };
    let edited = h.service.edit_channel(&voice, &user("alice"), changes).await.unwrap();
    assert!(!edited.channel.synced);

    let bob = h.service.view(id, &user("bob")).await.unwrap();
    assert_eq!(bob.channels.len(), 1);
    assert!(matches!(
        h.service.admit_voice(&voice, &user("bob")).await,
        Err(CommunitiesError::Refused(Refusal::Missing("view_channel")))
    ));
    assert!(!h.service.may_stay(&voice, &user("bob")).await.unwrap());
    assert!(matches!(
        h.service.channel(&voice, &user("bob")).await,
        Err(CommunitiesError::Forbidden)
    ));
    assert!(h.service.may_stay(&voice, &user("alice")).await.unwrap());
}

#[tokio::test]
async fn voice_admission_refuses_text_channels_unknown_channels_and_strangers() {
    let h = Harness::new();
    let view = h.community(&["bob"]).await;
    let text = channel_of(&view, ChannelKind::Text);

    assert!(matches!(
        h.service.admit_voice(&text, &user("bob")).await,
        Err(CommunitiesError::Refused(Refusal::NotVoice))
    ));
    let unknown = ChannelId::generate();
    assert!(matches!(
        h.service.admit_voice(&unknown, &user("bob")).await,
        Err(CommunitiesError::NotFound)
    ));
    assert!(!h.service.may_stay(&unknown, &user("bob")).await.unwrap(), "a deleted channel");
    let voice = channel_of(&view, ChannelKind::Voice);
    assert!(matches!(
        h.service.admit_voice(&voice, &user("carol")).await,
        Err(CommunitiesError::Forbidden)
    ));
}

#[tokio::test]
async fn channels_carry_their_plan_and_peer_to_peer_is_capped() {
    let h = Harness::new();
    let view = h.community(&[]).await;
    let id = &view.community.id;

    let p2p = h
        .service
        .create_channel(id, &user("alice"), voice_draft(MediaMode::PeerToPeer, Some(3)))
        .await
        .unwrap();
    assert_eq!(p2p.plan.effective_mode(), MediaMode::PeerToPeer);
    assert_eq!(p2p.plan.capacity(), Some(3));
    assert!(matches!(
        h.service
            .create_channel(id, &user("alice"), voice_draft(MediaMode::PeerToPeer, Some(5)))
            .await,
        Err(CommunitiesError::Invalid(_))
    ));
    let grant = h.service.admit_voice(&p2p.channel.id, &user("alice")).await.unwrap();
    assert_eq!(grant.plan(MeshPolicy::new(4)).capacity(), Some(3));
}

#[tokio::test]
async fn the_probe_reports_a_full_channel_by_the_roster() {
    let h = Harness::new();
    let view = h.community(&[]).await;
    let id = &view.community.id;
    let small = h
        .service
        .create_channel(id, &user("alice"), voice_draft(MediaMode::Server, Some(2)))
        .await
        .unwrap();
    let channel = small.channel.id.clone();
    assert!(h.service.probe_voice(&channel, &user("alice")).await.is_ok());

    for name in ["x", "y"] {
        let entry = VoiceEntry {
            channel_id: channel.clone(),
            session_id: SessionId::generate(),
            user_id: user(name).id,
            username: name.into(),
            voice: VoiceState::default(),
        };
        h.roster.put(id, &entry, Duration::from_secs(60)).await.unwrap();
    }

    assert!(matches!(
        h.service.probe_voice(&channel, &user("alice")).await,
        Err(CommunitiesError::Full)
    ));
    assert_eq!(h.service.voice_roster(id).await.unwrap().len(), 2);
}

#[tokio::test]
async fn categories_are_created_edited_and_deleted_leaving_their_channels() {
    let h = Harness::new();
    let view = h.community(&["bob"]).await;
    let id = &view.community.id;
    let category = h.service.create_category(id, &user("alice"), "Stage").await.unwrap();
    assert!(h.service.create_category(id, &user("bob"), "Mine").await.is_err());
    let draft = ChannelDraft {
        category_id: Some(category.id.clone()),
        ..voice_draft(MediaMode::Auto, None)
    };
    let channel = h.service.create_channel(id, &user("alice"), draft).await.unwrap();
    assert!(channel.channel.synced);

    h.service.delete_category(id, &user("alice"), &category.id).await.unwrap();

    let after = h.service.channel(&channel.channel.id, &user("alice")).await.unwrap();
    assert_eq!(after.channel.category_id, None);
}

#[tokio::test]
async fn the_feed_filter_holds_the_channels_the_member_may_view() {
    let h = Harness::new();
    let view = h.community(&["bob"]).await;
    let id = &view.community.id;
    let voice = channel_of(&view, ChannelKind::Voice);
    let hidden = ChannelId::generate();
    let entry = |channel_id: &ChannelId| VoiceEntry {
        channel_id: channel_id.clone(),
        session_id: SessionId::generate(),
        user_id: user("alice").id,
        username: "alice".into(),
        voice: VoiceState::default(),
    };

    let filter = h.service.feed_filter(id, &user("bob")).await.unwrap();

    assert_eq!(filter.snapshot(vec![entry(&voice), entry(&hidden)]).len(), 1);
    assert!(matches!(
        h.service.feed_filter(id, &user("carol")).await,
        Err(CommunitiesError::Forbidden)
    ));
}
