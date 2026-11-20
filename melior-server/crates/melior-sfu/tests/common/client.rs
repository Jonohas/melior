//! A browser on the application's page, as far as the HTTP and WebSocket tests
//! need one: JSON requests with the session cookie and the own origin, sockets,
//! and waiting for a message.

use super::{FRONTEND, RECHECK, TestServer};
use futures_util::{SinkExt, StreamExt};
use reqwest::header::{COOKIE, ORIGIN};
use reqwest::{Method, StatusCode};
use serde_json::{Value, json};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

pub type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// How long a test waits for one message.
pub const WAIT: Duration = Duration::from_secs(5);

/// Sends a request as a logged-in browser on the application's own page would.
/// The body comes back as JSON, or as a JSON string when it is plain text.
pub async fn call(
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

/// Joins a voice channel's room.
pub async fn join(server: &TestServer, channel: &str, cookie: &str) -> Socket {
    let (socket, _) =
        tokio_tungstenite::connect_async(server.ws_request(Some(channel), Some(cookie)))
            .await
            .expect("join");
    socket
}

/// The status a WebSocket upgrade was refused with; 101 if it was accepted.
pub async fn upgrade_status(server: &TestServer, path: &str, cookie: &str) -> StatusCode {
    let mut request = format!("ws://{}{path}", server.address).into_client_request().unwrap();
    request.headers_mut().insert("Cookie", HeaderValue::from_str(cookie).unwrap());
    request.headers_mut().insert("Origin", HeaderValue::from_static(FRONTEND));
    match tokio_tungstenite::connect_async(request).await {
        Ok(_) => StatusCode::SWITCHING_PROTOCOLS,
        Err(tokio_tungstenite::tungstenite::Error::Http(response)) => {
            StatusCode::from_u16(response.status().as_u16()).unwrap()
        }
        Err(error) => panic!("upgrade failed without a status: {error}"),
    }
}

/// Opens a community's feed.
pub async fn feed(server: &TestServer, community: &str, cookie: &str) -> Socket {
    let mut request = format!("ws://{}/ws/communities/{community}", server.address)
        .into_client_request()
        .unwrap();
    request.headers_mut().insert("Cookie", HeaderValue::from_str(cookie).unwrap());
    let (socket, _) = tokio_tungstenite::connect_async(request).await.expect("feed");
    socket
}

pub async fn send(socket: &mut Socket, channel: &str, payload: Value) {
    let message = json!({ "channel": channel, "payload": payload });
    socket.send(Message::text(message.to_string())).await.unwrap();
}

/// The payload of the next message on `channel`, skipping everything else.
pub async fn next_on(socket: &mut Socket, channel: &str) -> Value {
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

/// Whether a message on `channel` arrives within `within`.
pub async fn hears(socket: &mut Socket, channel: &str, within: Duration) -> bool {
    timeout(within, async {
        loop {
            match socket.next().await {
                Some(Ok(Message::Text(text))) => {
                    let message: Value = serde_json::from_str(text.as_str()).unwrap();
                    if message["channel"] == channel {
                        return;
                    }
                }
                Some(Ok(_)) => {}
                None | Some(Err(_)) => std::future::pending::<()>().await,
            }
        }
    })
    .await
    .is_ok()
}

/// Reads until the server says why it is closing the connection (`closed` on a
/// room socket, `removed` on a feed), then checks that it really closes it.
pub async fn closed_because(socket: &mut Socket) -> String {
    let deadline = RECHECK * 20;
    let reason = timeout(deadline, async {
        loop {
            let Some(Ok(Message::Text(text))) = socket.next().await else {
                panic!("socket ended without saying why");
            };
            let message: Value = serde_json::from_str(text.as_str()).unwrap();
            if message["channel"] == "closed" || message["channel"] == "removed" {
                return message["payload"]["reason"].as_str().unwrap().to_owned();
            }
        }
    })
    .await
    .expect("no closing message in time");

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
