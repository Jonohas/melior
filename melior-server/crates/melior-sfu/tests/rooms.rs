//! The room registry API, admission to restricted rooms, and the reasons the
//! server closes a connection, over real HTTP and WebSockets.

mod common;

use common::{FRONTEND, RECHECK, TestServer};
use futures_util::StreamExt;
use reqwest::header::{COOKIE, ORIGIN};
use reqwest::{Method, StatusCode};
use serde_json::{Value, json};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Sends a request as a logged-in browser on the application's own page would.
async fn call(
    server: &TestServer,
    method: Method,
    path: &str,
    cookie: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut request = reqwest::Client::new()
        .request(method, server.url(path))
        .header(COOKIE, cookie)
        .header(ORIGIN, FRONTEND);
    if let Some(body) = body {
        request = request.json(&body);
    }
    let response = request.send().await.unwrap();
    let status = response.status();
    let text = response.text().await.unwrap();
    (status, serde_json::from_str(&text).unwrap_or(Value::String(text)))
}

async fn join(server: &TestServer, channel: &str, cookie: &str) -> Socket {
    let (socket, _) =
        tokio_tungstenite::connect_async(server.ws_request(Some(channel), Some(cookie)))
            .await
            .expect("join");
    socket
}

async fn refused(server: &TestServer, channel: &str, cookie: &str) -> bool {
    tokio_tungstenite::connect_async(server.ws_request(Some(channel), Some(cookie))).await.is_err()
}

/// Reads until the server says why it is closing the connection, then checks that
/// it really closes it.
async fn closed_because(socket: &mut Socket) -> String {
    let deadline = RECHECK * 20;
    let reason = timeout(deadline, async {
        loop {
            let Some(Ok(Message::Text(text))) = socket.next().await else {
                panic!("socket ended without a closed message");
            };
            let message: Value = serde_json::from_str(text.as_str()).unwrap();
            if message["channel"] == "closed" {
                return message["payload"]["reason"].as_str().unwrap().to_owned();
            }
        }
    })
    .await
    .expect("no closed message in time");

    let ended = timeout(deadline, async {
        loop {
            match socket.next().await {
                None | Some(Err(_) | Ok(Message::Close(_))) => return,
                Some(Ok(_)) => {}
            }
        }
    })
    .await;
    assert!(ended.is_ok(), "the server must close the socket after saying why");
    reason
}

#[tokio::test]
async fn joining_registers_an_open_room_that_its_owner_can_restrict() {
    let server = TestServer::start().await;
    let alice = server.login("alice", &["member"]).await;
    let bob = server.login("bob", &["member"]).await;
    let carol = server.login("carol", &["member"]).await;

    // Nobody has been here yet.
    let (status, _) = call(&server, Method::GET, "/api/rooms/board", &alice, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // The first to join owns the room; it is open to members.
    let _alice_socket = join(&server, "board", &alice).await;
    let (status, room) = call(&server, Method::GET, "/api/rooms/board", &bob, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        room,
        json!({ "channel_id": "board", "owner": "alice", "restricted": false, "members": [], "is_owner": false })
    );

    // Only the owner changes it.
    let restrict = json!({ "restricted": true, "members": ["Bob"] });
    let (status, _) =
        call(&server, Method::PUT, "/api/rooms/board", &bob, Some(restrict.clone())).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, room) =
        call(&server, Method::PUT, "/api/rooms/board", &alice, Some(restrict)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        room,
        json!({ "channel_id": "board", "owner": "alice", "restricted": true, "members": ["bob"], "is_owner": true })
    );

    // The invited get in, the others are refused at the door and cannot look.
    let _bob_socket = join(&server, "board", &bob).await;
    assert!(refused(&server, "board", &carol).await);
    let (status, _) = call(&server, Method::GET, "/api/rooms/board", &carol, None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Each sees their own rooms.
    let (_, rooms) = call(&server, Method::GET, "/api/rooms", &bob, None).await;
    assert_eq!(rooms.as_array().unwrap().len(), 1);
    let (_, rooms) = call(&server, Method::GET, "/api/rooms", &carol, None).await;
    assert_eq!(rooms, json!([]));

    // Deleting unregisters the channel: it is open again.
    let (status, _) = call(&server, Method::DELETE, "/api/rooms/board", &bob, None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = call(&server, Method::DELETE, "/api/rooms/board", &alice, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let _carol_socket = join(&server, "board", &carol).await;
}

#[tokio::test]
async fn room_requests_are_validated_and_need_a_session_a_role_and_the_own_origin() {
    let server = TestServer::start().await;
    let alice = server.login("alice", &["member"]).await;
    let guest = server.login("guest", &[]).await;
    let open = json!({ "restricted": false, "members": [] });

    let (status, message) = call(
        &server,
        Method::PUT,
        "/api/rooms/board",
        &alice,
        Some(json!({ "restricted": true, "members": ["not a name"] })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(message.as_str().unwrap().contains("username"));

    let (status, _) =
        call(&server, Method::PUT, "/api/rooms/not%20valid", &alice, Some(open.clone())).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let too_many: Vec<String> = (0..201).map(|n| format!("user{n}")).collect();
    let (status, _) = call(
        &server,
        Method::PUT,
        "/api/rooms/board",
        &alice,
        Some(json!({ "restricted": true, "members": too_many })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) =
        call(&server, Method::PUT, "/api/rooms/board", &guest, Some(open.clone())).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = call(&server, Method::GET, "/api/rooms", &guest, None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let anonymous = reqwest::Client::new().get(server.url("/api/rooms")).send().await.unwrap();
    assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);

    // Another site's page, riding on alice's cookie, changes nothing.
    for method in [Method::PUT, Method::DELETE] {
        let foreign = reqwest::Client::new()
            .request(method, server.url("/api/rooms/board"))
            .header(COOKIE, &alice)
            .header(ORIGIN, "http://evil.example")
            .json(&open)
            .send()
            .await
            .unwrap();
        assert_eq!(foreign.status(), StatusCode::FORBIDDEN);
    }
    let (status, _) = call(&server, Method::GET, "/api/rooms/board", &alice, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "nothing was registered by any of the above");
}

#[tokio::test]
async fn a_participant_who_loses_access_is_removed() {
    let server = TestServer::start().await;
    let alice = server.login("alice", &["member"]).await;
    let carol = server.login("carol", &["member"]).await;
    let mut alice_socket = join(&server, "board", &alice).await;
    let mut carol_socket = join(&server, "board", &carol).await;

    let (status, _) = call(
        &server,
        Method::PUT,
        "/api/rooms/board",
        &alice,
        Some(json!({ "restricted": true, "members": [] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    assert_eq!(closed_because(&mut carol_socket).await, "removed");

    // The owner stays, and is told that carol left.
    let told = timeout(Duration::from_secs(5), async {
        loop {
            let Some(Ok(Message::Text(text))) = alice_socket.next().await else {
                panic!("alice was disconnected too");
            };
            let message: Value = serde_json::from_str(text.as_str()).unwrap();
            assert_ne!(message["channel"], "closed");
            if message["channel"] == "peer_left" {
                return;
            }
        }
    })
    .await;
    assert!(told.is_ok());
}

#[tokio::test]
async fn logging_out_closes_the_users_open_connections() {
    let server = TestServer::start().await;
    let alice = server.login("alice", &["member"]).await;
    let mut socket = join(&server, "board", &alice).await;

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
    let mut socket = join(&server, "board", &alice).await;

    server.leases.move_rooms_elsewhere();

    assert_eq!(closed_because(&mut socket).await, "room_moved");
    assert!(refused(&server, "board", &alice).await, "and new joins are sent away as well");
}
