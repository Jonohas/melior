//! End-to-end key messages travel through the real server, unread, to the room only.

mod common;

use common::{TestServer, next_json};
use futures_util::SinkExt;
use serde_json::{Value, json};
use std::time::Duration;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn join(server: &TestServer, channel: &str, username: &str) -> (Socket, String) {
    server.voice_channel(channel).await;
    let cookie = server.login(username, &["member"]).await;
    let request = server.ws_request(Some(channel), Some(&cookie));
    let (mut socket, _) = tokio_tungstenite::connect_async(request).await.expect("connect");
    let hello = next_json(&mut socket).await;
    assert_eq!(hello["channel"], "session");
    (socket, hello["payload"]["session_id"].as_str().unwrap().to_owned())
}

/// The next message on `channel`, skipping the rest; None if none comes in a second.
async fn next_on(socket: &mut Socket, channel: &str) -> Option<Value> {
    timeout(Duration::from_secs(1), async {
        loop {
            let message = next_json(socket).await;
            if message["channel"] == channel {
                return message;
            }
        }
    })
    .await
    .ok()
}

async fn send_key(socket: &mut Socket, to: &str, payload: &str) {
    let message = json!({ "channel": "e2ee", "payload": { "to": to, "payload": payload } });
    socket.send(Message::text(message.to_string())).await.unwrap();
}

#[tokio::test]
async fn a_key_message_reaches_its_peer_with_the_sender_named_by_the_server() {
    let server = TestServer::start().await;
    let (mut alice, alice_id) = join(&server, "keys", "alice").await;
    let (mut bob, bob_id) = join(&server, "keys", "bob").await;

    send_key(&mut alice, &bob_id, "aGVsbG8").await;
    let relayed = next_on(&mut bob, "e2ee").await.expect("relayed");
    assert_eq!(relayed["payload"], json!({ "from": alice_id, "payload": "aGVsbG8" }));
}

#[tokio::test]
async fn a_key_message_never_leaves_the_room_and_a_bad_one_is_dropped_quietly() {
    let server = TestServer::start().await;
    let (mut alice, _) = join(&server, "keys", "alice").await;
    let (mut bob, bob_id) = join(&server, "keys", "bob").await;
    let (mut carol, carol_id) = join(&server, "elsewhere", "carol").await;

    send_key(&mut alice, &carol_id, "aGVsbG8").await;
    send_key(&mut alice, &bob_id, "not base64!").await;
    assert!(next_on(&mut carol, "e2ee").await.is_none(), "not into another room");
    assert!(next_on(&mut bob, "e2ee").await.is_none(), "not a malformed one");
    assert!(next_on(&mut alice, "error").await.is_none(), "and no error for either");
}
