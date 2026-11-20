//! Room chat over real WebSockets, and its history over real HTTP.

mod common;

use common::{FRONTEND, TestServer};
use futures_util::{SinkExt, StreamExt};
use reqwest::header::{COOKIE, ORIGIN};
use reqwest::{Method, StatusCode};
use serde_json::{Value, json};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

const WAIT: Duration = Duration::from_secs(5);

async fn join(server: &TestServer, channel: &str, cookie: &str) -> Socket {
    let (socket, _) =
        tokio_tungstenite::connect_async(server.ws_request(Some(channel), Some(cookie)))
            .await
            .expect("join");
    socket
}

async fn say(socket: &mut Socket, payload: Value) {
    let message = json!({ "channel": "chat_send", "payload": payload });
    socket.send(Message::text(message.to_string())).await.unwrap();
}

/// The next message on `channel`, skipping signalling the test is not about.
async fn next_on(socket: &mut Socket, channel: &str) -> Value {
    timeout(WAIT, async {
        loop {
            let Some(Ok(Message::Text(text))) = socket.next().await else {
                panic!("socket ended while waiting for {channel}");
            };
            let message: Value = serde_json::from_str(text.as_str()).unwrap();
            if message["channel"] == channel {
                return message["payload"].clone();
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("no {channel} message in time"))
}

async fn get(server: &TestServer, path: &str, cookie: &str) -> (StatusCode, Value) {
    let response = reqwest::Client::new()
        .request(Method::GET, server.url(path))
        .header(COOKIE, cookie)
        .header(ORIGIN, FRONTEND)
        .send()
        .await
        .unwrap();
    let status = response.status();
    let text = response.text().await.unwrap();
    (status, serde_json::from_str(&text).unwrap_or(Value::String(text)))
}

#[tokio::test]
async fn a_message_reaches_everyone_in_the_room_and_only_the_author_gets_its_client_id() {
    let server = TestServer::start().await;
    let alice = server.login("alice", &["member"]).await;
    let bob = server.login("bob", &["member"]).await;
    let mut alice_socket = join(&server, "chat", &alice).await;
    let mut bob_socket = join(&server, "chat", &bob).await;
    // Both are in once alice has heard about bob.
    next_on(&mut alice_socket, "peer_joined").await;

    say(&mut alice_socket, json!({ "client_id": "c1", "text": "  hello bob  " })).await;

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
    let alice = server.login("alice", &["member"]).await;
    let bob = server.login("bob", &["member"]).await;
    let mut alice_socket = join(&server, "chat", &alice).await;
    let mut bob_socket = join(&server, "chat", &bob).await;
    next_on(&mut alice_socket, "peer_joined").await;

    say(&mut alice_socket, json!({ "client_id": "c1", "text": "   " })).await;
    let refusal = next_on(&mut alice_socket, "chat_error").await;
    assert_eq!(refusal["client_id"], "c1");
    assert_eq!(refusal["message"], "a message cannot be empty");

    // Nothing reached bob: the next thing he hears is the message that follows.
    say(&mut alice_socket, json!({ "text": "second" })).await;
    assert_eq!(next_on(&mut bob_socket, "chat").await["text"], "second");
}

#[tokio::test]
async fn history_returns_the_room_chat_in_pages_and_follows_room_access() {
    let server = TestServer::start().await;
    let alice = server.login("alice", &["member"]).await;
    let bob = server.login("bob", &["member"]).await;
    let carol = server.login("carol", &["member"]).await;
    let mut alice_socket = join(&server, "chat", &alice).await;
    for n in 1..=3 {
        say(&mut alice_socket, json!({ "text": format!("m{n}") })).await;
        next_on(&mut alice_socket, "chat").await;
    }

    let (status, page) = get(&server, "/api/rooms/chat/messages?limit=2", &bob).await;
    assert_eq!(status, StatusCode::OK);
    let texts: Vec<&str> =
        page["messages"].as_array().unwrap().iter().map(|m| m["text"].as_str().unwrap()).collect();
    assert_eq!(texts, ["m2", "m3"]);
    assert_eq!(page["has_more"], true);

    let before = page["messages"][0]["id"].as_i64().unwrap();
    let (_, older) =
        get(&server, &format!("/api/rooms/chat/messages?limit=2&before={before}"), &bob).await;
    assert_eq!(older["messages"][0]["text"], "m1");
    assert_eq!(older["has_more"], false);

    // Restricting the room closes its history to anyone not invited.
    let restrict = json!({ "restricted": true, "members": ["bob"] });
    let response = reqwest::Client::new()
        .put(server.url("/api/rooms/chat"))
        .header(COOKIE, &alice)
        .header(ORIGIN, FRONTEND)
        .json(&restrict)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let (status, _) = get(&server, "/api/rooms/chat/messages", &carol).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = get(&server, "/api/rooms/chat/messages", &bob).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn history_needs_a_login_a_registered_room_and_a_valid_channel() {
    let server = TestServer::start().await;
    let alice = server.login("alice", &["member"]).await;

    let (status, _) = get(&server, "/api/rooms/chat/messages", &alice).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = get(&server, "/api/rooms/bad%20name/messages", &alice).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let response =
        reqwest::Client::new().get(server.url("/api/rooms/chat/messages")).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_message_that_is_not_json_does_not_end_the_connection() {
    let server = TestServer::start().await;
    let alice = server.login("alice", &["member"]).await;
    let mut socket = join(&server, "chat", &alice).await;

    socket.send(Message::text("not json")).await.unwrap();
    assert!(next_on(&mut socket, "error").await["message"].as_str().unwrap().contains("invalid"));

    say(&mut socket, json!({ "text": "still here" })).await;
    assert_eq!(next_on(&mut socket, "chat").await["text"], "still here");
}
