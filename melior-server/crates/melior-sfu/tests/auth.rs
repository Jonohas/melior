//! The login endpoints and the session gate, over real HTTP.

mod common;

use common::{FRONTEND, SESSION_COOKIE, TestServer};
use reqwest::header::{COOKIE, LOCATION, SET_COOKIE};
use reqwest::{Response, StatusCode};
use tokio_tungstenite::tungstenite::http::HeaderValue;

fn client() -> reqwest::Client {
    reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).build().unwrap()
}

fn location(response: &Response) -> &str {
    response.headers()[LOCATION].to_str().unwrap()
}

/// The `Set-Cookie` header for `name`, if the response has one.
fn set_cookie<'a>(response: &'a Response, name: &str) -> Option<&'a str> {
    response
        .headers()
        .get_all(SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find(|value| value.starts_with(&format!("{name}=")))
}

/// `name=value` from a `Set-Cookie` header, as a browser would send it back.
fn pair(set_cookie: &str) -> &str {
    set_cookie.split(';').next().unwrap()
}

#[tokio::test]
async fn login_round_trip_opens_a_session_and_logout_ends_it() {
    let server = TestServer::start().await;
    let http = client();

    // Not logged in.
    let me = http.get(server.url("/api/identity/me")).send().await.unwrap();
    assert_eq!(me.status(), StatusCode::UNAUTHORIZED);

    // /login sends the browser to the identity provider and hands it the login state.
    let login = http
        .get(server.url("/login?redirectTo=%2Frooms%2Fstandup%3Frelay%3Dtrue"))
        .send()
        .await
        .unwrap();
    assert_eq!(login.status(), StatusCode::FOUND);
    assert_eq!(
        location(&login),
        format!(
            "http://idp.test/auth?redirect_uri={FRONTEND}/signin&state=/rooms/standup?relay=true"
        )
    );
    let login_state = set_cookie(&login, "bff_pkce").expect("login state cookie");
    assert!(login_state.contains("HttpOnly"));
    assert!(login_state.contains("SameSite=Lax"));
    assert!(login_state.contains("Path=/"));

    // The callback opens the session and returns to where the user was going.
    let signin = http
        .get(server.url("/signin?code=alice:member&state=%2Frooms%2Fstandup%3Frelay%3Dtrue"))
        .header(COOKIE, pair(login_state))
        .send()
        .await
        .unwrap();
    assert_eq!(signin.status(), StatusCode::FOUND);
    assert_eq!(location(&signin), format!("{FRONTEND}/rooms/standup?relay=true"));
    let session = set_cookie(&signin, SESSION_COOKIE).expect("session cookie");
    assert!(session.contains("HttpOnly"));
    assert!(session.contains("SameSite=Lax"));
    assert!(session.contains("Max-Age=3600"));
    assert!(!session.contains("Secure"), "the test frontend is plain http");
    let completed = set_cookie(&signin, "bff_login_completed").expect("completion marker");
    assert!(completed.starts_with("bff_login_completed=1;"));
    assert!(!completed.contains("HttpOnly"), "the frontend must be able to read it");
    assert!(
        set_cookie(&signin, "bff_pkce").unwrap().contains("Max-Age=0"),
        "login state is single use"
    );

    let me = http
        .get(server.url("/api/identity/me"))
        .header(COOKIE, pair(session))
        .send()
        .await
        .unwrap();
    assert_eq!(me.status(), StatusCode::OK);
    assert_eq!(
        me.json::<serde_json::Value>().await.unwrap(),
        serde_json::json!({
            "subject": "sub-alice",
            "username": "alice",
            "email": "alice@example.test",
            "roles": ["member"]
        })
    );

    // Logout goes through the identity provider and the session is gone afterwards.
    let signout =
        http.get(server.url("/signout")).header(COOKIE, pair(session)).send().await.unwrap();
    assert_eq!(signout.status(), StatusCode::FOUND);
    assert_eq!(location(&signout), format!("http://idp.test/logout?hint=id-alice&to={FRONTEND}/"));
    assert!(set_cookie(&signout, SESSION_COOKIE).unwrap().contains("Max-Age=0"));
    assert!(server.sessions.is_empty());

    let me = http
        .get(server.url("/api/identity/me"))
        .header(COOKIE, pair(session))
        .send()
        .await
        .unwrap();
    assert_eq!(me.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn callbacks_that_do_not_belong_to_this_browser_open_no_session() {
    let server = TestServer::start().await;
    let http = client();

    // No code at all: start over.
    let response = http.get(server.url("/signin")).send().await.unwrap();
    assert_eq!(location(&response), format!("{FRONTEND}/login"));

    // A code without the login state cookie (replayed in another browser).
    let response = http.get(server.url("/signin?code=alice:member&state=/")).send().await.unwrap();
    assert_eq!(location(&response), format!("{FRONTEND}/error?reason=login-failed"));
    assert!(set_cookie(&response, SESSION_COOKIE).is_none());

    // The wrong login state.
    let response = http
        .get(server.url("/signin?code=alice:member&state=/"))
        .header(COOKIE, "bff_pkce=other.nonce")
        .send()
        .await
        .unwrap();
    assert_eq!(location(&response), format!("{FRONTEND}/error?reason=login-failed"));
    assert!(server.sessions.is_empty());

    // An open redirect attempt through `state` lands on the application root.
    let response = http
        .get(server.url("/signin?code=alice:member&state=https%3A%2F%2Fevil.example%2F"))
        .header(COOKIE, "bff_pkce=verifier.nonce")
        .send()
        .await
        .unwrap();
    assert_eq!(location(&response), format!("{FRONTEND}/"));
}

#[tokio::test]
async fn garbage_and_unknown_session_cookies_are_rejected_and_cleared() {
    let server = TestServer::start().await;
    let http = client();
    let unknown = melior_domain::AuthSessionId::generate();

    for value in ["not-a-session-id", unknown.as_str()] {
        let me = http
            .get(server.url("/api/identity/me"))
            .header(COOKIE, format!("{SESSION_COOKIE}={value}"))
            .send()
            .await
            .unwrap();
        assert_eq!(me.status(), StatusCode::UNAUTHORIZED);
        assert!(
            set_cookie(&me, SESSION_COOKIE).unwrap().contains("Max-Age=0"),
            "a dead cookie is removed"
        );
    }
}

#[tokio::test]
async fn users_without_a_melior_role_are_forbidden_everywhere() {
    let server = TestServer::start().await;
    let guest = server.login("guest", &[]).await;

    let me =
        client().get(server.url("/api/identity/me")).header(COOKIE, &guest).send().await.unwrap();
    assert_eq!(me.status(), StatusCode::FORBIDDEN);

    let ws =
        tokio_tungstenite::connect_async(server.ws_request(Some("general"), Some(&guest))).await;
    assert!(ws.is_err(), "no socket for a user without a role");
}

#[tokio::test]
async fn the_websocket_needs_a_session_and_the_applications_own_origin() {
    let server = TestServer::start().await;
    let alice = server.login("alice", &["member"]).await;

    // No session.
    assert!(
        tokio_tungstenite::connect_async(server.ws_request(Some("general"), None)).await.is_err()
    );

    // A page on another site, riding on the user's cookie.
    let mut foreign = server.ws_request(Some("general"), Some(&alice));
    foreign.headers_mut().insert("Origin", HeaderValue::from_static("http://evil.example"));
    assert!(tokio_tungstenite::connect_async(foreign).await.is_err());

    // The application's own page.
    let mut own = server.ws_request(Some("general"), Some(&alice));
    own.headers_mut().insert("Origin", HeaderValue::from_static(FRONTEND));
    assert!(tokio_tungstenite::connect_async(own).await.is_ok());
}

#[tokio::test]
async fn health_needs_no_session() {
    let server = TestServer::start().await;
    let health = client().get(server.url("/api/health")).send().await.unwrap();
    assert_eq!(health.status(), StatusCode::OK);
    assert_eq!(
        health.json::<serde_json::Value>().await.unwrap(),
        serde_json::json!({ "status": "ok" })
    );
}
