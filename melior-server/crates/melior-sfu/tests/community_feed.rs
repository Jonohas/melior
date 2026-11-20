//! The community feed over real WebSockets: the snapshot, voice presence, text
//! channel messages and structure changes, filtered by `view_channel`, and the
//! end of the feed for a member who is out.

mod common;

use common::TestServer;
use common::client::{
    Socket, call, closed_because, feed, hears, join, next_on, send, upgrade_status,
};
use reqwest::{Method, StatusCode};
use serde_json::{Value, json};
use std::time::Duration;

struct Town {
    id: String,
    voice: String,
    text: String,
}

async fn town(server: &TestServer, alice: &str, joiners: &[&str]) -> Town {
    let (_, view) =
        call(server, Method::POST, "/api/communities", alice, Some(json!({ "name": "Town" })))
            .await;
    let id = view["id"].as_str().unwrap().to_owned();
    let (_, invite) = call(
        server,
        Method::POST,
        &format!("/api/communities/{id}/invites"),
        alice,
        Some(json!({})),
    )
    .await;
    let code = invite["code"].as_str().unwrap();
    for joiner in joiners {
        call(server, Method::POST, &format!("/api/invites/{code}"), joiner, None).await;
    }
    let of = |kind: &str| {
        let channel =
            view["channels"].as_array().unwrap().iter().find(|c| c["kind"] == kind).unwrap();
        channel["id"].as_str().unwrap().to_owned()
    };
    Town { voice: of("voice"), text: of("text"), id }
}

/// A second voice channel only alice may view.
async fn hidden_channel(server: &TestServer, town: &Town, alice: &str) -> String {
    let body = json!({ "name": "Back room", "kind": "voice" });
    let path = format!("/api/communities/{}/channels", town.id);
    let (_, channel) = call(server, Method::POST, &path, alice, Some(body)).await;
    let id = channel["id"].as_str().unwrap().to_owned();
    let hide = json!({ "overrides": [{ "target": { "kind": "role", "id": town.id }, "deny": ["view_channel"] }] });
    call(server, Method::PATCH, &format!("/api/channels/{id}"), alice, Some(hide)).await;
    id
}

async fn snapshot(socket: &mut Socket) -> Vec<Value> {
    next_on(socket, "snapshot").await["voice"].as_array().unwrap().clone()
}

#[tokio::test]
async fn the_feed_starts_with_a_snapshot_and_follows_voice_presence() {
    let server = TestServer::start().await;
    let alice = server.login("alice", &["member"]).await;
    let bob = server.login("bob", &["member"]).await;
    let town = town(&server, &alice, &[&bob]).await;
    let mut alice_room = join(&server, &town.voice, &alice).await;
    let alice_session = next_on(&mut alice_room, "session").await["session_id"].clone();

    let mut bob_feed = feed(&server, &town.id, &bob).await;
    // The presence queue may not have stored alice yet: wait for her in a snapshot.
    let mut present = snapshot(&mut bob_feed).await;
    for _ in 0..20 {
        if !present.is_empty() {
            break;
        }
        present = snapshot(&mut bob_feed).await;
    }
    assert_eq!(present.len(), 1);
    assert_eq!(present[0]["session_id"], alice_session);
    assert_eq!(present[0]["username"], "alice");
    assert_eq!(present[0]["channel_id"], town.voice.as_str());

    send(
        &mut alice_room,
        "voice_state",
        json!({ "muted": false, "camera": true, "screen": false, "speaking": true }),
    )
    .await;
    let updated = next_on(&mut bob_feed, "voice_updated").await;
    assert_eq!(
        (updated["camera"].as_bool(), updated["speaking"].as_bool()),
        (Some(true), Some(true))
    );

    let bob_room = join(&server, &town.voice, &bob).await;
    let joined = next_on(&mut bob_feed, "voice_joined").await;
    assert_eq!(joined["username"], "bob");
    drop(bob_room);
    let left = next_on(&mut bob_feed, "voice_left").await;
    assert_eq!(left["session_id"], joined["session_id"]);
    assert_eq!(left["channel_id"], town.voice.as_str());
}

#[tokio::test]
async fn the_feed_carries_only_channels_the_member_may_view() {
    let server = TestServer::start().await;
    let alice = server.login("alice", &["member"]).await;
    let bob = server.login("bob", &["member"]).await;
    let town = town(&server, &alice, &[&bob]).await;
    let back_room = hidden_channel(&server, &town, &alice).await;
    let mut bob_feed = feed(&server, &town.id, &bob).await;
    snapshot(&mut bob_feed).await;

    let _alice_room = join(&server, &back_room, &alice).await;
    assert!(!hears(&mut bob_feed, "voice_joined", Duration::from_millis(400)).await);
    let mut alice_feed = feed(&server, &town.id, &alice).await;
    let mut seen = snapshot(&mut alice_feed).await;
    for _ in 0..20 {
        if !seen.is_empty() {
            break;
        }
        seen = snapshot(&mut alice_feed).await;
    }
    assert_eq!(seen[0]["channel_id"], back_room.as_str(), "alice sees herself there");
    assert!(snapshot(&mut bob_feed).await.is_empty(), "bob's snapshots leave it out");

    let path = format!("/api/channels/{}/messages", town.text);
    let (status, _) =
        call(&server, Method::POST, &path, &alice, Some(json!({ "text": "hello" }))).await;
    assert_eq!(status, StatusCode::OK);
    let message = next_on(&mut bob_feed, "message").await;
    assert_eq!(message["channel_id"], town.text.as_str());
    assert_eq!(message["message"]["text"], "hello");
    assert!(message["message"].get("client_id").is_none());
}

#[tokio::test]
async fn structure_changes_are_announced_and_members_who_are_out_are_told_why() {
    let server = TestServer::start().await;
    let alice = server.login("alice", &["member"]).await;
    let bob = server.login("bob", &["member"]).await;
    let carol = server.login("carol", &["member"]).await;
    let town = town(&server, &alice, &[&bob, &carol]).await;
    let mut bob_feed = feed(&server, &town.id, &bob).await;
    let mut carol_feed = feed(&server, &town.id, &carol).await;

    let path = format!("/api/communities/{}/categories", town.id);
    call(&server, Method::POST, &path, &alice, Some(json!({ "name": "Stage" }))).await;
    assert_eq!(next_on(&mut bob_feed, "structure_changed").await, json!({}));

    let kick = format!("/api/communities/{}/members/sub-carol", town.id);
    call(&server, Method::DELETE, &kick, &alice, None).await;
    assert_eq!(closed_because(&mut carol_feed).await, "kicked");

    let leave = format!("/api/communities/{}/members/sub-bob", town.id);
    let mut bob_second = feed(&server, &town.id, &bob).await;
    call(&server, Method::DELETE, &leave, &bob, None).await;
    assert_eq!(closed_because(&mut bob_second).await, "left");
    assert_eq!(closed_because(&mut bob_feed).await, "left");

    let mut alice_feed = feed(&server, &town.id, &alice).await;
    call(&server, Method::DELETE, &format!("/api/communities/{}", town.id), &alice, None).await;
    assert_eq!(closed_because(&mut alice_feed).await, "deleted");
}

#[tokio::test]
async fn members_hear_when_someone_joins_or_is_kicked() {
    let server = TestServer::start().await;
    let alice = server.login("alice", &["member"]).await;
    let bob = server.login("bob", &["member"]).await;
    let carol = server.login("carol", &["member"]).await;
    let town = town(&server, &alice, &[&carol]).await;
    let mut alice_feed = feed(&server, &town.id, &alice).await;

    let invites = format!("/api/communities/{}/invites", town.id);
    let (_, invite) = call(&server, Method::POST, &invites, &alice, Some(json!({}))).await;
    let redeem = format!("/api/invites/{}", invite["code"].as_str().unwrap());
    call(&server, Method::POST, &redeem, &bob, None).await;
    assert_eq!(next_on(&mut alice_feed, "members_changed").await, json!({}));

    let kick = format!("/api/communities/{}/members/sub-carol", town.id);
    call(&server, Method::DELETE, &kick, &alice, None).await;
    assert_eq!(next_on(&mut alice_feed, "members_changed").await, json!({}));
}

#[tokio::test]
async fn the_feed_is_for_members_only() {
    let server = TestServer::start().await;
    let alice = server.login("alice", &["member"]).await;
    let bob = server.login("bob", &["member"]).await;
    let town = town(&server, &alice, &[]).await;

    let path = |id: &str| format!("/ws/communities/{id}");
    assert_eq!(upgrade_status(&server, &path(&town.id), &bob).await, StatusCode::FORBIDDEN);
    let unknown = "00000000-0000-4000-8000-000000000000";
    assert_eq!(upgrade_status(&server, &path(unknown), &alice).await, StatusCode::NOT_FOUND);
    assert_eq!(upgrade_status(&server, &path("nope"), &alice).await, StatusCode::BAD_REQUEST);
    assert_eq!(
        upgrade_status(&server, &path(&town.id), &alice).await,
        StatusCode::SWITCHING_PROTOCOLS
    );
}
