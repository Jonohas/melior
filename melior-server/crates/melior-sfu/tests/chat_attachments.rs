//! Images and GIFs in chat over real HTTP and WebSockets: uploading an image,
//! sending it in a room and a text channel, fetching it, and the GIF settings.

mod common;

use common::client::{call, join, next_on, send};
use common::{FRONTEND, TestServer};
use reqwest::header::{CONTENT_TYPE, COOKIE, ORIGIN, X_CONTENT_TYPE_OPTIONS};
use reqwest::{Method, StatusCode};
use serde_json::{Value, json};

const PNG: &[u8] = b"\x89PNG\r\n\x1a\nnot-really-a-png";
const GIF_URL: &str = "https://static.klipy.com/ii/abc/14/af/um0L4dFH.gif";

async fn upload(
    server: &TestServer,
    channel: &str,
    cookie: &str,
    bytes: Vec<u8>,
) -> (StatusCode, Value) {
    let response = reqwest::Client::new()
        .post(server.url(&format!("/api/channels/{channel}/attachments")))
        .header(COOKIE, cookie)
        .header(ORIGIN, FRONTEND)
        .header(CONTENT_TYPE, "image/png")
        .body(bytes)
        .send()
        .await
        .unwrap();
    let status = response.status();
    let text = response.text().await.unwrap();
    (status, serde_json::from_str(&text).unwrap_or(Value::String(text)))
}

#[tokio::test]
async fn an_uploaded_image_is_sent_in_the_room_and_served_to_readers() {
    let server = TestServer::start().await;
    server.voice_channel("pics").await;
    let alice = server.login("alice", &["member"]).await;
    let bob = server.login("bob", &["member"]).await;

    let (status, uploaded) = upload(&server, "pics", &alice, PNG.to_vec()).await;
    assert_eq!(status, StatusCode::OK, "{uploaded}");
    assert_eq!(uploaded["content_type"], "image/png");
    assert_eq!(uploaded["size"], PNG.len());
    let id = uploaded["id"].as_str().unwrap().to_owned();

    let mut socket = join(&server, "pics", &alice).await;
    let attachment = json!({ "kind": "image", "id": id });
    send(&mut socket, "chat_send", json!({ "text": "", "attachment": attachment })).await;
    let message = next_on(&mut socket, "chat").await;
    assert_eq!(message["text"], "");
    assert_eq!(
        message["attachment"],
        json!({ "kind": "image", "id": id, "content_type": "image/png" })
    );

    let response = reqwest::Client::new()
        .get(server.url(&format!("/api/attachments/{id}")))
        .header(COOKIE, &bob)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[CONTENT_TYPE], "image/png");
    assert_eq!(response.headers()[X_CONTENT_TYPE_OPTIONS], "nosniff");
    assert_eq!(response.bytes().await.unwrap().as_ref(), PNG);
}

#[tokio::test]
async fn uploads_are_checked_by_content_size_origin_and_login() {
    let server = TestServer::start().await;
    server.voice_channel("pics").await;
    let alice = server.login("alice", &["member"]).await;

    let (status, body) = upload(&server, "pics", &alice, b"<svg onload=alert(1)/>".to_vec()).await;
    assert_eq!(
        (status, body.as_str()),
        (StatusCode::BAD_REQUEST, Some("an image must be a PNG, JPEG, GIF or WebP file"))
    );

    let mut too_large = PNG.to_vec();
    too_large.resize(10 * 1024 * 1024 + 1, 0);
    let (status, _) = upload(&server, "pics", &alice, too_large).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);

    let (status, _) = upload(&server, "nowhere", &alice, PNG.to_vec()).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let foreign = reqwest::Client::new()
        .post(server.url("/api/channels/pics/attachments"))
        .header(COOKIE, &alice)
        .header(ORIGIN, "https://evil.example")
        .body(PNG.to_vec())
        .send()
        .await
        .unwrap();
    assert_eq!(foreign.status(), StatusCode::FORBIDDEN);

    let anonymous =
        reqwest::Client::new().get(server.url("/api/gifs/config")).send().await.unwrap();
    assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn images_follow_the_channel_permissions_and_unknown_ids_are_404() {
    let server = TestServer::start().await;
    let alice = server.login("alice", &["member"]).await;
    let (_, view) =
        call(&server, Method::POST, "/api/communities", &alice, Some(json!({ "name": "Town" })))
            .await;
    let text = view["channels"].as_array().unwrap().iter().find(|c| c["kind"] == "text").unwrap();
    let channel = text["id"].as_str().unwrap();
    let (_, uploaded) = upload(&server, channel, &alice, PNG.to_vec()).await;
    let image = format!("/api/attachments/{}", uploaded["id"].as_str().unwrap());

    let stranger = server.login("mallory", &["member"]).await;
    let (status, _) = upload(&server, channel, &stranger, PNG.to_vec()).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = call(&server, Method::GET, &image, &stranger, None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = call(&server, Method::GET, "/api/attachments/not-an-id", &alice, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let unknown = "/api/attachments/0b6c1a5e-7f43-4c1e-9b8f-1d2e3f4a5b6c";
    let (status, _) = call(&server, Method::GET, unknown, &alice, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_text_channel_takes_gifs_from_klipy_only() {
    let server = TestServer::start().await;
    let alice = server.login("alice", &["member"]).await;
    let (_, view) =
        call(&server, Method::POST, "/api/communities", &alice, Some(json!({ "name": "Town" })))
            .await;
    let text = view["channels"].as_array().unwrap().iter().find(|c| c["kind"] == "text").unwrap();
    let path = format!("/api/channels/{}/messages", text["id"].as_str().unwrap());

    let gif = json!({ "kind": "gif", "url": GIF_URL, "width": 498, "height": 280 });
    let (status, message) =
        call(&server, Method::POST, &path, &alice, Some(json!({ "text": "", "attachment": gif })))
            .await;
    assert_eq!(status, StatusCode::OK, "{message}");
    assert_eq!(message["attachment"], gif);
    let (_, page) = call(&server, Method::GET, &path, &alice, None).await;
    assert_eq!(page["messages"][0]["attachment"], gif);

    let elsewhere =
        json!({ "kind": "gif", "url": "https://evil.example/a.gif", "width": 1, "height": 1 });
    let (status, body) = call(
        &server,
        Method::POST,
        &path,
        &alice,
        Some(json!({ "text": "", "attachment": elsewhere })),
    )
    .await;
    assert_eq!(
        (status, body.as_str()),
        (StatusCode::BAD_REQUEST, Some("a GIF must come from KLIPY"))
    );

    let (status, config) = call(&server, Method::GET, "/api/gifs/config", &alice, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(config, json!({ "klipy_app_key": "test-klipy-key" }));
}
