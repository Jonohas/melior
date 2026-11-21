//! The community API over real HTTP: communities, invites, roles, channels and
//! the checks in front of all of them.

mod common;

use common::TestServer;
use common::client::call;
use reqwest::header::{COOKIE, ORIGIN};
use reqwest::{Method, StatusCode};
use serde_json::{Value, json};

async fn create(server: &TestServer, cookie: &str, name: &str) -> Value {
    let (status, view) =
        call(server, Method::POST, "/api/communities", cookie, Some(json!({ "name": name }))).await;
    assert_eq!(status, StatusCode::OK, "{view}");
    view
}

fn id(value: &Value) -> String {
    value["id"].as_str().unwrap().to_owned()
}

/// Creates an invite of `community` as `cookie` and redeems it as each of `joiners`.
async fn invite(server: &TestServer, community: &str, cookie: &str, joiners: &[&str]) -> String {
    let path = format!("/api/communities/{community}/invites");
    let (status, invite) = call(server, Method::POST, &path, cookie, Some(json!({}))).await;
    assert_eq!(status, StatusCode::OK, "{invite}");
    let code = invite["code"].as_str().unwrap().to_owned();
    for joiner in joiners {
        let (status, joined) =
            call(server, Method::POST, &format!("/api/invites/{code}"), joiner, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(joined, json!({ "community_id": community }));
    }
    code
}

#[tokio::test]
async fn a_new_community_is_seeded_and_listed_for_its_members_only() {
    let server = TestServer::start().await;
    let alice = server.login("alice", &["member"]).await;
    let bob = server.login("bob", &["member"]).await;

    let view = create(&server, &alice, " Town ").await;

    assert_eq!(view["name"], "Town");
    assert_eq!(view["owner_username"], "alice");
    assert_eq!(view["me"]["is_owner"], true);
    assert_eq!(view["me"]["rank"], u32::MAX);
    assert_eq!(view["roles"][0]["name"], "@everyone");
    assert_eq!(view["roles"][0]["id"], view["id"]);
    let categories: Vec<&str> = view["categories"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    assert_eq!(categories, ["Text channels", "Voice channels"]);
    let channels: Vec<(&str, &str)> = view["channels"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| (c["name"].as_str().unwrap(), c["kind"].as_str().unwrap()))
        .collect();
    assert_eq!(channels, [("general", "text"), ("General", "voice")]);
    let voice = &view["channels"][1];
    assert_eq!(voice["effective_mode"], "server", "mesh is off on the test server");
    assert_eq!(voice["media_warning"], Value::Null);

    let (_, mine) = call(&server, Method::GET, "/api/communities", &alice, None).await;
    let community = id(&view);
    assert!(mine.as_array().unwrap().iter().any(|c| c["id"] == community.as_str()));
    let (status, _) =
        call(&server, Method::GET, &format!("/api/communities/{community}"), &bob, None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn an_invite_is_previewed_redeemed_counted_and_used_up() {
    let server = TestServer::start().await;
    let alice = server.login("alice", &["member"]).await;
    let bob = server.login("bob", &["member"]).await;
    let carol = server.login("carol", &["member"]).await;
    let community = id(&create(&server, &alice, "Town").await);
    let path = format!("/api/communities/{community}/invites");
    let (_, invite) = call(
        &server,
        Method::POST,
        &path,
        &alice,
        Some(json!({ "max_uses": 1, "max_age_seconds": 3600 })),
    )
    .await;
    let code = invite["code"].as_str().unwrap();
    assert_eq!(invite["uses"], 0);
    assert_eq!(invite["created_by"], "alice");
    assert!(invite["expires_at_ms"].as_i64().unwrap() > 0);

    let invite_path = format!("/api/invites/{code}");
    let (status, preview) = call(&server, Method::GET, &invite_path, &bob, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(preview["community"]["name"], "Town");
    assert_eq!(
        (preview["member_count"].as_u64(), preview["usable"].as_bool()),
        (Some(1), Some(true))
    );

    let (status, _) = call(&server, Method::POST, &invite_path, &bob, None).await;
    assert_eq!(status, StatusCode::OK);
    let (status, again) = call(&server, Method::POST, &invite_path, &bob, None).await;
    assert_eq!(
        (status, again["community_id"].as_str()),
        (StatusCode::OK, Some(community.as_str()))
    );
    let (status, _) = call(&server, Method::POST, &invite_path, &carol, None).await;
    assert_eq!(status, StatusCode::GONE);

    let (status, _) = call(&server, Method::GET, &path, &bob, None).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "listing invites needs manage_channels");
    let (_, list) = call(&server, Method::GET, &path, &alice, None).await;
    assert_eq!(list[0]["uses"], 1);
    let (status, _) = call(&server, Method::GET, "/api/invites/aB3dE5fG7h", &bob, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = call(&server, Method::GET, "/api/invites/short", &bob, None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = call(&server, Method::DELETE, &invite_path, &bob, None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = call(&server, Method::DELETE, &invite_path, &alice, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn roles_follow_the_hierarchy_and_grant_what_they_hold() {
    let server = TestServer::start().await;
    let alice = server.login("alice", &["member"]).await;
    let bob = server.login("bob", &["member"]).await;
    let carol = server.login("carol", &["member"]).await;
    let community = id(&create(&server, &alice, "Town").await);
    invite(&server, &community, &alice, &[&bob, &carol]).await;
    let roles = format!("/api/communities/{community}/roles");

    let body =
        json!({ "name": "mod", "color": 255, "permissions": ["manage_roles", "kick_members"] });
    let (status, _) = call(&server, Method::POST, &roles, &bob, Some(body.clone())).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, moderator) = call(&server, Method::POST, &roles, &alice, Some(body)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(moderator["position"], 1);
    let (_, helper) =
        call(&server, Method::POST, &roles, &alice, Some(json!({ "name": "helper" }))).await;
    assert_eq!(helper["position"], 2);
    // Put the moderator above the helper.
    let edit = format!("{roles}/{}", id(&moderator));
    let (status, _) =
        call(&server, Method::PATCH, &edit, &alice, Some(json!({ "position": 3 }))).await;
    assert_eq!(status, StatusCode::OK);

    let member_roles =
        |user: &str| format!("/api/communities/{community}/members/sub-{user}/roles");
    let (status, bob_member) = call(
        &server,
        Method::PUT,
        &member_roles("bob"),
        &alice,
        Some(json!({ "role_ids": [id(&moderator)] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(bob_member["role_ids"], json!([id(&moderator)]));

    // bob may now hand out the helper role, but not his own or anything above it.
    let (status, _) = call(
        &server,
        Method::PUT,
        &member_roles("carol"),
        &bob,
        Some(json!({ "role_ids": [id(&helper)] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, refusal) = call(
        &server,
        Method::PUT,
        &member_roles("carol"),
        &bob,
        Some(json!({ "role_ids": [id(&moderator)] })),
    )
    .await;
    assert_eq!(
        (status, refusal.as_str()),
        (StatusCode::FORBIDDEN, Some("that role is not below your highest role"))
    );
    let (status, _) = call(
        &server,
        Method::POST,
        &roles,
        &bob,
        Some(json!({ "name": "x", "permissions": ["administrator"] })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "cannot grant what he does not have");
    let (status, _) = call(
        &server,
        Method::POST,
        &roles,
        &bob,
        Some(json!({ "name": "x", "permissions": ["fly"] })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let everyone = format!("{roles}/{community}");
    let (status, _) = call(&server, Method::DELETE, &everyone, &alice, None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Kicks follow rank: bob may kick carol, not alice.
    let members = |user: &str| format!("/api/communities/{community}/members/sub-{user}");
    let (status, _) = call(&server, Method::DELETE, &members("alice"), &bob, None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = call(&server, Method::DELETE, &members("carol"), &bob, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, list) =
        call(&server, Method::GET, &format!("/api/communities/{community}/members"), &alice, None)
            .await;
    assert_eq!(list.as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn channels_and_categories_are_managed_and_hidden_ones_stay_hidden() {
    let server = TestServer::start().await;
    let alice = server.login("alice", &["member"]).await;
    let bob = server.login("bob", &["member"]).await;
    let community = id(&create(&server, &alice, "Town").await);
    invite(&server, &community, &alice, &[&bob]).await;
    let channels = format!("/api/communities/{community}/channels");

    let body =
        json!({ "name": "Stand-up", "kind": "voice", "media_mode": "server", "user_limit": 5 });
    let (status, _) = call(&server, Method::POST, &channels, &bob, Some(body.clone())).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, channel) = call(&server, Method::POST, &channels, &alice, Some(body)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        (channel["effective_mode"].as_str(), channel["effective_limit"].as_u64()),
        (Some("server"), Some(5))
    );
    let channel_path = format!("/api/channels/{}", id(&channel));

    let hide = json!({ "overrides": [{ "target": { "kind": "role", "id": community }, "deny": ["view_channel"] }] });
    let (status, edited) = call(&server, Method::PATCH, &channel_path, &alice, Some(hide)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(edited["synced"], false);
    let (status, _) = call(&server, Method::GET, &channel_path, &bob, None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (_, bob_view) =
        call(&server, Method::GET, &format!("/api/communities/{community}"), &bob, None).await;
    assert!(bob_view["channels"].as_array().unwrap().iter().all(|c| c["id"] != channel["id"]));

    let p2p = json!({ "media_mode": "peer_to_peer", "user_limit": 9 });
    let (status, _) = call(&server, Method::PATCH, &channel_path, &alice, Some(p2p)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "above the largest mesh");

    let categories = format!("/api/communities/{community}/categories");
    let (_, category) =
        call(&server, Method::POST, &categories, &alice, Some(json!({ "name": "Stage" }))).await;
    let moved = json!({ "category_id": id(&category), "synced": true });
    let (_, edited) = call(&server, Method::PATCH, &channel_path, &alice, Some(moved)).await;
    assert_eq!(edited["category_id"], category["id"]);
    let (status, _) = call(&server, Method::GET, &channel_path, &bob, None).await;
    assert_eq!(status, StatusCode::OK, "synced with an open category");

    let (status, _) =
        call(&server, Method::DELETE, &format!("{categories}/{}", id(&category)), &alice, None)
            .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, after) = call(&server, Method::GET, &channel_path, &alice, None).await;
    assert_eq!(after["category_id"], Value::Null);
    let (status, _) = call(&server, Method::DELETE, &channel_path, &alice, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = call(&server, Method::GET, &channel_path, &alice, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn renaming_and_deleting_belong_to_the_owner() {
    let server = TestServer::start().await;
    let alice = server.login("alice", &["member"]).await;
    let bob = server.login("bob", &["member"]).await;
    let community = id(&create(&server, &alice, "Town").await);
    invite(&server, &community, &alice, &[&bob]).await;
    let path = format!("/api/communities/{community}");

    let (status, _) =
        call(&server, Method::PATCH, &path, &bob, Some(json!({ "name": "Mine" }))).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, view) =
        call(&server, Method::PATCH, &path, &alice, Some(json!({ "name": "City" }))).await;
    assert_eq!((status, view["name"].as_str()), (StatusCode::OK, Some("City")));
    let (status, _) =
        call(&server, Method::DELETE, &format!("{path}/members/sub-alice"), &alice, None).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "the owner cannot leave");
    let (status, _) = call(&server, Method::DELETE, &path, &bob, None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = call(&server, Method::DELETE, &path, &alice, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = call(&server, Method::GET, &path, &alice, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn requests_need_a_session_a_role_valid_input_and_the_own_origin() {
    let server = TestServer::start().await;
    let alice = server.login("alice", &["member"]).await;
    let guest = server.login("guest", &[]).await;

    let (status, _) = call(&server, Method::GET, "/api/communities", &guest, None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) =
        call(&server, Method::POST, "/api/communities", &guest, Some(json!({ "name": "x" }))).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let anonymous =
        reqwest::Client::new().get(server.url("/api/communities")).send().await.unwrap();
    assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);

    for body in [json!({ "name": "" }), json!({ "nom": "x" })] {
        let (status, _) = call(&server, Method::POST, "/api/communities", &alice, Some(body)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
    let (status, _) = call(&server, Method::GET, "/api/communities/not-an-id", &alice, None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let foreign = reqwest::Client::new()
        .post(server.url("/api/communities"))
        .header(COOKIE, &alice)
        .header(ORIGIN, "http://evil.example")
        .json(&json!({ "name": "x" }))
        .send()
        .await
        .unwrap();
    assert_eq!(foreign.status(), StatusCode::FORBIDDEN);
    let (_, mine) = call(&server, Method::GET, "/api/communities", &alice, None).await;
    assert_eq!(mine.as_array().unwrap().len(), 1, "only the lobby: nothing was created");
}
