//! Chat over real WebSockets and HTTP: in a voice channel's room, in a text
//! channel, and the history, each following the channel's permissions.

mod common;

use common::TestServer;
use common::client::{call, join, next_on, send};
use futures_util::SinkExt;
use reqwest::{Method, StatusCode};
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::Message;

/// Sets the overrides of a lobby channel, as the lobby's owner.
async fn set_overrides(server: &TestServer, channel: &str, overrides: Value) {
    let owner = server.login("lobby-owner", &["member"]).await;
    let path = format!("/api/channels/{channel}");
    let (status, body) =
        call(server, Method::PATCH, &path, &owner, Some(json!({ "overrides": overrides }))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

fn deny_member(username: &str, permissions: &[&str]) -> Value {
    json!({ "target": { "kind": "member", "id": format!("sub-{username}") }, "deny": permissions })
}

#[tokio::test]
async fn a_message_reaches_everyone_in_the_room_and_only_the_author_gets_its_client_id() {
    let server = TestServer::start().await;
    server.voice_channel("chat").await;
    let alice = server.login("alice", &["member"]).await;
    let bob = server.login("bob", &["member"]).await;
    let mut alice_socket = join(&server, "chat", &alice).await;
    let mut bob_socket = join(&server, "chat", &bob).await;
    // Both are in once alice has heard about bob.
    next_on(&mut alice_socket, "peer_joined").await;

    send(&mut alice_socket, "chat_send", json!({ "client_id": "c1", "text": "  hello bob  " }))
        .await;

    let theirs = next_on(&mut bob_socket, "chat").await;
    assert_eq!(theirs["username"], "alice");
    assert_eq!(theirs["text"], "hello bob");
    assert!(theirs.get("client_id").is_none());
    let mine = next_on(&mut alice_socket, "chat").await;
    assert_eq!(mine["client_id"], "c1");
    assert_eq!(mine["id"], theirs["id"]);
}

#[tokio::test]
async fn a_refused_message_is_reported_to_its_sender_alone() {
    let server = TestServer::start().await;
    server.voice_channel("chat").await;
    let alice = server.login("alice", &["member"]).await;
    let bob = server.login("bob", &["member"]).await;
    let mut alice_socket = join(&server, "chat", &alice).await;
    let mut bob_socket = join(&server, "chat", &bob).await;
    next_on(&mut alice_socket, "peer_joined").await;

    send(&mut alice_socket, "chat_send", json!({ "client_id": "c1", "text": "   " })).await;
    let refusal = next_on(&mut alice_socket, "chat_error").await;
    assert_eq!(refusal["client_id"], "c1");
    assert_eq!(refusal["message"], "a message cannot be empty");

    // Nothing reached bob: the next thing he hears is the message that follows.
    send(&mut alice_socket, "chat_send", json!({ "text": "second" })).await;
    assert_eq!(next_on(&mut bob_socket, "chat").await["text"], "second");
}

#[tokio::test]
async fn sending_in_a_room_needs_send_messages() {
    let server = TestServer::start().await;
    server.voice_channel("chat").await;
    set_overrides(&server, "chat", json!([deny_member("bob", &["send_messages"])])).await;
    let bob = server.login("bob", &["member"]).await;
    let mut socket = join(&server, "chat", &bob).await;

    send(&mut socket, "chat_send", json!({ "client_id": "c9", "text": "hi" })).await;

    let refusal = next_on(&mut socket, "chat_error").await;
    assert_eq!(refusal["message"], "you may not send messages in this channel");
    assert_eq!(refusal["client_id"], "c9");
}

#[tokio::test]
async fn history_returns_the_channel_chat_in_pages_and_needs_read_history() {
    let server = TestServer::start().await;
    server.voice_channel("chat").await;
    let alice = server.login("alice", &["member"]).await;
    let bob = server.login("bob", &["member"]).await;
    let carol = server.login("carol", &["member"]).await;
    let mut alice_socket = join(&server, "chat", &alice).await;
    for n in 1..=3 {
        send(&mut alice_socket, "chat_send", json!({ "text": format!("m{n}") })).await;
        next_on(&mut alice_socket, "chat").await;
    }

    let (status, page) =
        call(&server, Method::GET, "/api/channels/chat/messages?limit=2", &bob, None).await;
    assert_eq!(status, StatusCode::OK);
    let texts: Vec<&str> =
        page["messages"].as_array().unwrap().iter().map(|m| m["text"].as_str().unwrap()).collect();
    assert_eq!(texts, ["m2", "m3"]);
    assert_eq!(page["has_more"], true);

    let before = page["messages"][0]["id"].as_i64().unwrap();
    let path = format!("/api/channels/chat/messages?limit=2&before={before}");
    let (_, older) = call(&server, Method::GET, &path, &bob, None).await;
    assert_eq!(older["messages"][0]["text"], "m1");
    assert_eq!(older["has_more"], false);

    set_overrides(&server, "chat", json!([deny_member("carol", &["read_history"])])).await;
    let (status, body) =
        call(&server, Method::GET, "/api/channels/chat/messages", &carol, None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body, "you need the read_history permission");
    let (status, _) = call(&server, Method::GET, "/api/channels/chat/messages", &bob, None).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn history_needs_a_login_a_known_channel_and_a_valid_id() {
    let server = TestServer::start().await;
    let alice = server.login("alice", &["member"]).await;

    let (status, _) =
        call(&server, Method::GET, "/api/channels/nowhere/messages", &alice, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) =
        call(&server, Method::GET, "/api/channels/bad%20name/messages", &alice, None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let response = reqwest::Client::new()
        .get(server.url("/api/channels/general/messages"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_message_that_is_not_json_does_not_end_the_connection() {
    let server = TestServer::start().await;
    let alice = server.login("alice", &["member"]).await;
    let mut socket = join(&server, "general", &alice).await;

    socket.send(Message::text("not json")).await.unwrap();
    assert!(next_on(&mut socket, "error").await["message"].as_str().unwrap().contains("invalid"));

    send(&mut socket, "chat_send", json!({ "text": "still here" })).await;
    assert_eq!(next_on(&mut socket, "chat").await["text"], "still here");
}

#[tokio::test]
async fn posting_to_a_text_channel_saves_the_message_and_voice_channels_refuse_posts() {
    let server = TestServer::start().await;
    let alice = server.login("alice", &["member"]).await;
    let (_, view) =
        call(&server, Method::POST, "/api/communities", &alice, Some(json!({ "name": "Town" })))
            .await;
    let text = view["channels"].as_array().unwrap().iter().find(|c| c["kind"] == "text").unwrap();
    let voice = view["channels"].as_array().unwrap().iter().find(|c| c["kind"] == "voice").unwrap();
    let path =
        |channel: &Value| format!("/api/channels/{}/messages", channel["id"].as_str().unwrap());

    let (status, message) = call(
        &server,
        Method::POST,
        &path(text),
        &alice,
        Some(json!({ "text": " hi ", "client_id": "k" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!((message["text"].as_str(), message["client_id"].as_str()), (Some("hi"), Some("k")));
    let (_, page) = call(&server, Method::GET, &path(text), &alice, None).await;
    assert_eq!(page["messages"][0]["text"], "hi");

    let (status, _) =
        call(&server, Method::POST, &path(voice), &alice, Some(json!({ "text": "hi" }))).await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, _) =
        call(&server, Method::POST, &path(text), &alice, Some(json!({ "text": "" }))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let bob = server.login("bob", &["member"]).await;
    let (status, _) =
        call(&server, Method::POST, &path(text), &bob, Some(json!({ "text": "hi" }))).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "not a member");
}
