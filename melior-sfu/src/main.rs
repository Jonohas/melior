mod structs;
mod shared;
mod streaming;
mod prelude;
mod error;
mod websocket;
mod handlers;

use axum::{
    response::IntoResponse,
    routing::get,
    Router,
};
use axum::extract::WebSocketUpgrade;
use crate::websocket::handler::{ws_handler};

#[tokio::main]
async fn main() {
    let app = Router::new().route(
        "/ws",
        get({
            move |ws: WebSocketUpgrade| ws_handler(ws)
        }),
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:5000")
        .await
        .unwrap();

    axum::serve(listener, app).await.unwrap();
}