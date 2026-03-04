use super::message::IncomingMessage;
use super::router::WebsocketMessageRouter;
use crate::structs::app_state::AppState;
use axum::extract::ws::{Message, WebSocket};
use axum::extract::{Query, State, WebSocketUpgrade};
use axum::response::IntoResponse;
use serde::Deserialize;
use tokio::sync::mpsc;

#[derive(Deserialize)]
pub struct WsQueryParams {
    session_id: String,
    channel_id: String,
}

pub async fn ws_handler(
    Query(params): Query<WsQueryParams>,
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, params.session_id, params.channel_id, state))
}

async fn handle_socket(
    mut socket: WebSocket,
    session_id: String,
    _channel_id: String,
    state: AppState,
) {
    let (tx, mut rx) = mpsc::channel::<Message>(100);

    state.client_registry.register(session_id.clone(), tx).await;

    loop {
        tokio::select! {
            msg = socket.recv() => {
                let msg = match msg {
                    Some(Ok(Message::Text(text))) => text,
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => {
                        state.client_registry.unregister(&session_id).await;
                        return;
                    }
                    _ => continue,
                };

                let incoming: IncomingMessage = match serde_json::from_str(&msg) {
                    Ok(m) => m,
                    Err(e) => {
                        eprintln!("Invalid message: {e}");
                        continue;
                    }
                };
                WebsocketMessageRouter::handle(&mut socket, &incoming, &state).await
            }
            msg = rx.recv() => {
                let msg = match msg {
                    Some(m) => m,
                    None => {
                        state.client_registry.unregister(&session_id).await;
                        return;
                    }
                };
                if socket.send(msg).await.is_err() {
                    state.client_registry.unregister(&session_id).await;
                    return;
                }
            }
        }
    }
}
