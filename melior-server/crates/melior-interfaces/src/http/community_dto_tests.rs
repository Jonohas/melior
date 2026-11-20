//! The community API's response shapes.

use super::*;
use melior_domain::{
    ChannelMedia, CommunityId, CommunityState, InviteSettings, MediaMode, MeshPolicy, Name, Rank,
    RoomPlan, User,
};
use serde_json::{Value, json};

fn owner() -> User {
    User::new(UserId::new("sub-alice"), "alice", None, [User::ROLE_MEMBER.to_owned()])
}

fn state() -> (CommunityState, Member) {
    CommunityState::create(CommunityId::generate(), Name::parse("Town").unwrap(), &owner())
}

fn wire<T: Serialize>(value: T) -> Value {
    serde_json::to_value(value).unwrap()
}

#[test]
fn a_voice_channel_view_shows_its_settings_and_what_they_come_to() {
    let (state, _) = state();
    let mut channel = state.channels[1].clone();
    channel.media = ChannelMedia { mode: MediaMode::PeerToPeer, user_limit: Some(6) };
    let view = ChannelView {
        community_id: state.id().clone(),
        plan: RoomPlan::resolve(channel.media, MeshPolicy::new(4)),
        channel: channel.clone(),
        permissions: Permissions::VIEW_CHANNEL.union(Permissions::CONNECT),
    };

    assert_eq!(
        wire(ChannelViewDto::from(view)),
        json!({
            "id": channel.id.as_str(),
            "community_id": state.id().as_str(),
            "name": "General",
            "topic": "",
            "kind": "voice",
            "category_id": channel.category_id.as_ref().unwrap().as_str(),
            "position": 1,
            "synced": true,
            "overrides": [],
            "media_mode": "peer_to_peer",
            "user_limit": 6,
            "effective_mode": "peer_to_peer",
            "effective_limit": 4,
            "media_warning": "limit_clamped",
            "my_permissions": ["view_channel", "connect"]
        })
    );
}

#[test]
fn a_text_channel_view_reports_no_room() {
    let (state, _) = state();
    let view = ChannelView {
        community_id: state.id().clone(),
        plan: RoomPlan::resolve(ChannelMedia::default(), MeshPolicy::OFF),
        channel: state.channels[0].clone(),
        permissions: Permissions::ALL,
    };
    let dto = wire(ChannelViewDto::from(view));
    assert_eq!(dto["kind"], "text");
    assert_eq!(dto["effective_mode"], "auto");
    assert_eq!(dto["effective_limit"], Value::Null);
    assert_eq!(dto["media_warning"], Value::Null);
}

#[test]
fn a_community_view_carries_the_viewer_roles_and_categories() {
    let (state, member) = state();
    let view = CommunityView {
        community: state.community.clone(),
        me: MemberView { member, permissions: Permissions::ALL, rank: Rank::Owner, is_owner: true },
        roles: state.roles.clone(),
        categories: state.categories.clone(),
        channels: Vec::new(),
    };
    let dto = wire(CommunityViewDto::from(view));
    assert_eq!(dto["name"], "Town");
    assert_eq!(dto["owner_id"], "sub-alice");
    assert_eq!(dto["owner_username"], "alice");
    assert_eq!(
        dto["me"],
        json!({
            "user_id": "sub-alice", "role_ids": [], "permissions": Permissions::ALL.names(),
            "rank": u32::MAX, "is_owner": true
        })
    );
    assert_eq!(
        dto["roles"],
        json!([{
            "id": state.id().as_str(), "name": "@everyone", "color": null, "position": 0,
            "permissions": ["view_channel", "connect", "send_messages", "read_history", "create_invite"]
        }])
    );
    assert_eq!(dto["categories"][0]["name"], "Text channels");
    assert_eq!(dto["categories"][0]["overrides"], json!([]));
}

#[test]
fn overrides_round_trip_through_the_wire_shape() {
    let entry = Override {
        target: OverrideTarget::Member(UserId::new("sub-bob")),
        allow: Permissions::CONNECT,
        deny: Permissions::VIEW_CHANNEL.union(Permissions::SEND_MESSAGES),
    };
    let dto = OverrideDto::from(&entry);
    assert_eq!(
        wire(dto.clone()),
        json!({
            "target": { "kind": "member", "id": "sub-bob" },
            "allow": ["connect"],
            "deny": ["view_channel", "send_messages"]
        })
    );
    assert_eq!(dto.into_override().unwrap(), entry);
}

#[test]
fn invites_members_and_previews_have_their_documented_shape() {
    let (state, member) = state();
    let settings = InviteSettings { max_age_seconds: None, max_uses: Some(5) };
    let invite = Invite::new(state.id().clone(), &member, settings, 0);
    assert_eq!(
        wire(InviteDto::from(invite.clone())),
        json!({
            "code": invite.code.as_str(), "community_id": state.id().as_str(), "created_by": "alice",
            "expires_at_ms": null, "max_uses": 5, "uses": 0
        })
    );
    assert_eq!(
        wire(MemberDto::from(member)),
        json!({ "user_id": "sub-alice", "username": "alice", "role_ids": [] })
    );
    let preview = InvitePreview {
        invite: invite.clone(),
        community: state.community.clone(),
        member_count: 3,
        usable: true,
    };
    assert_eq!(
        wire(InvitePreviewDto::from(preview)),
        json!({
            "code": invite.code.as_str(),
            "community": { "id": state.id().as_str(), "name": "Town" },
            "member_count": 3,
            "usable": true
        })
    );
    assert_eq!(
        wire(CommunitySummaryDto::from(state.community.clone())),
        json!({ "id": state.id().as_str(), "name": "Town", "owner_username": "alice" })
    );
}
