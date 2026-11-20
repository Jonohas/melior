//! The room socket of a voice channel over real WebSockets: who gets in, who is
//! removed on the re-check, full channels, voice states, and the reasons the
//! server closes a connection.

mod common;

use common::TestServer;
use common::client::{call, closed_because, join, next_on, send, upgrade_status};
use reqwest::header::COOKIE;
use reqwest::{Method, StatusCode};
use serde_json::{Value, json};

/// alice's community with every joiner invited. Returns it and its voice channel.
async fn town(server: &TestServer, alice: &str, joiners: &[&str]) -> (Value, String) {
    let (_, view) =
        call(server, Method::POST, "/api/communities", alice, Some(json!({ "name": "Town" })))
            .await;
    let community = view["id"].as_str().unwrap();
    let path = format!("/api/communities/{community}/invites");
    let (_, invite) = call(server, Method::POST, &path, alice, None).await;
    let code = invite["code"].as_str().unwrap();
    for joiner in joiners {
        call(server, Method::POST, &format!("/api/invites/{code}"), joiner, None).await;
    }
    let voice = view["channels"].as_array().unwrap().iter().find(|c| c["kind"] == "voice").unwrap();
    let voice = voice["id"].as_str().unwrap().to_owned();
    (view, voice)
}

fn ws(channel: &str) -> String {
    format!("/ws?channel_id={channel}")
}

#[tokio::test]
async fn the_upgrade_refuses_unknown_channels_outsiders_text_channels_and_hidden_ones() {
    let server = TestServer::start().await;
    let alice = server.login("alice", &["member"]).await;
    let bob = server.login("bob", &["member"]).await;
    let carol = server.login("carol", &["member"]).await;
    let guest = server.login("guest", &[]).await;
    let (view, voice) = town(&server, &alice, &[&bob]).await;
    let text = view["channels"].as_array().unwrap().iter().find(|c| c["kind"] == "text").unwrap();

    assert_eq!(upgrade_status(&server, &ws("nowhere"), &alice).await, StatusCode::NOT_FOUND);
    assert_eq!(upgrade_status(&server, "/ws", &alice).await, StatusCode::BAD_REQUEST);
    assert_eq!(upgrade_status(&server, &ws("not%20valid"), &alice).await, StatusCode::BAD_REQUEST);
    assert_eq!(upgrade_status(&server, &ws(&voice), &carol).await, StatusCode::FORBIDDEN);
    assert_eq!(upgrade_status(&server, &ws(&voice), &guest).await, StatusCode::FORBIDDEN);
    let text = text["id"].as_str().unwrap();
    assert_eq!(upgrade_status(&server, &ws(text), &bob).await, StatusCode::FORBIDDEN);
    assert_eq!(upgrade_status(&server, &ws(&voice), &bob).await, StatusCode::SWITCHING_PROTOCOLS);

    let community = view["id"].as_str().unwrap();
    let hide = json!({ "overrides": [
        { "target": { "kind": "role", "id": community }, "allow": ["view_channel"], "deny": ["connect"] }
    ] });
    call(&server, Method::PATCH, &format!("/api/channels/{voice}"), &alice, Some(hide)).await;
    assert_eq!(upgrade_status(&server, &ws(&voice), &bob).await, StatusCode::FORBIDDEN);
    let (status, _) =
        call(&server, Method::GET, &format!("/api/channels/{voice}/admission"), &bob, None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(upgrade_status(&server, &ws(&voice), &alice).await, StatusCode::SWITCHING_PROTOCOLS);
}

#[tokio::test]
async fn a_full_channel_refuses_the_next_participant() {
    let server = TestServer::start().await;
    let alice = server.login("alice", &["member"]).await;
    let bob = server.login("bob", &["member"]).await;
    let carol = server.login("carol", &["member"]).await;
    let (view, _) = town(&server, &alice, &[&bob, &carol]).await;
    let path = format!("/api/communities/{}/channels", view["id"].as_str().unwrap());
    let body = json!({ "name": "Pair", "kind": "voice", "user_limit": 2 });
    let (_, pair) = call(&server, Method::POST, &path, &alice, Some(body)).await;
    let pair = pair["id"].as_str().unwrap();
    let admission = format!("/api/channels/{pair}/admission");

    let (status, _) = call(&server, Method::GET, &admission, &carol, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let mut alice_socket = join(&server, pair, &alice).await;
    let _bob_socket = join(&server, pair, &bob).await;
    next_on(&mut alice_socket, "peer_joined").await;

    assert_eq!(upgrade_status(&server, &ws(pair), &carol).await, StatusCode::CONFLICT);
    // The probe goes by the roster, which the presence queue fills a moment later.
    let mut probe = StatusCode::OK;
    for _ in 0..50 {
        probe = call(&server, Method::GET, &admission, &carol, None).await.0;
        if probe == StatusCode::CONFLICT {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert_eq!(probe, StatusCode::CONFLICT);
}

#[tokio::test]
async fn a_participant_who_loses_access_is_removed_and_the_others_stay() {
    let server = TestServer::start().await;
    let alice = server.login("alice", &["member"]).await;
    let bob = server.login("bob", &["member"]).await;
    let (view, voice) = town(&server, &alice, &[&bob]).await;
    let mut alice_socket = join(&server, &voice, &alice).await;
    let mut bob_socket = join(&server, &voice, &bob).await;
    next_on(&mut alice_socket, "peer_joined").await;

    let community = view["id"].as_str().unwrap();
    let hide = json!({ "overrides": [
        { "target": { "kind": "role", "id": community }, "deny": ["view_channel"] }
    ] });
    call(&server, Method::PATCH, &format!("/api/channels/{voice}"), &alice, Some(hide)).await;

    assert_eq!(closed_because(&mut bob_socket).await, "removed");
    assert!(next_on(&mut alice_socket, "peer_left").await["session_id"].is_string());
}

#[tokio::test]
async fn a_kicked_member_and_everyone_in_a_deleted_channel_are_removed() {
    let server = TestServer::start().await;
    let alice = server.login("alice", &["member"]).await;
    let bob = server.login("bob", &["member"]).await;
    let (view, voice) = town(&server, &alice, &[&bob]).await;
    let community = view["id"].as_str().unwrap();
    let mut alice_socket = join(&server, &voice, &alice).await;
    let mut bob_socket = join(&server, &voice, &bob).await;

    let kick = format!("/api/communities/{community}/members/sub-bob");
    let (status, _) = call(&server, Method::DELETE, &kick, &alice, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(closed_because(&mut bob_socket).await, "removed");

    let (status, _) =
        call(&server, Method::DELETE, &format!("/api/channels/{voice}"), &alice, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(closed_because(&mut alice_socket).await, "removed");
}

#[tokio::test]
async fn a_voice_state_reaches_everyone_and_newcomers_see_it() {
    let server = TestServer::start().await;
    let alice = server.login("alice", &["member"]).await;
    let bob = server.login("bob", &["member"]).await;
    let carol = server.login("carol", &["member"]).await;
    let mut alice_socket = join(&server, "general", &alice).await;
    let alice_id = next_on(&mut alice_socket, "session").await["session_id"].clone();
    let mut bob_socket = join(&server, "general", &bob).await;
    let joined = next_on(&mut alice_socket, "peer_joined").await;
    assert_eq!(
        joined["voice"],
        json!({ "muted": false, "camera": false, "screen": false, "speaking": false })
    );

    // A screen claim without a screen track is not accepted; speaking while muted neither.
    let claim = json!({ "muted": true, "camera": true, "screen": true, "speaking": true });
    send(&mut alice_socket, "voice_state", claim).await;

    let accepted = json!({ "session_id": alice_id, "muted": true, "camera": true, "screen": false, "speaking": false });
    assert_eq!(next_on(&mut alice_socket, "voice_state").await, accepted);
    assert_eq!(next_on(&mut bob_socket, "voice_state").await, accepted);

    let mut carol_socket = join(&server, "general", &carol).await;
    let session = next_on(&mut carol_socket, "session").await;
    let participants = session["participants"].as_array().unwrap();
    let alice_entry = participants.iter().find(|p| p["session_id"] == alice_id).unwrap();
    assert_eq!(
        alice_entry["voice"],
        json!({ "muted": true, "camera": true, "screen": false, "speaking": false })
    );
}

#[tokio::test]
async fn logging_out_closes_the_users_open_connections() {
    let server = TestServer::start().await;
    let alice = server.login("alice", &["member"]).await;
    let mut socket = join(&server, "general", &alice).await;

    let signout = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap()
        .get(server.url("/signout"))
        .header(COOKIE, &alice)
        .send()
        .await
        .unwrap();
    assert_eq!(signout.status(), StatusCode::FOUND);

    assert_eq!(closed_because(&mut socket).await, "session_ended");
}

#[tokio::test]
async fn a_room_that_moved_to_another_instance_sends_its_participants_there() {
    let server = TestServer::start().await;
    let alice = server.login("alice", &["member"]).await;
    let mut socket = join(&server, "general", &alice).await;

    server.leases.move_rooms_elsewhere();

    assert_eq!(closed_because(&mut socket).await, "room_moved");
    assert_eq!(
        upgrade_status(&server, &ws("general"), &alice).await,
        StatusCode::SERVICE_UNAVAILABLE,
        "and new joins are sent away as well"
    );
}
