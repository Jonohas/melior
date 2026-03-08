use crate::structs::app_state::AppState;
use axum::extract::ws::WebSocket;
use serde::Deserialize;
use serde_json::Value;

pub trait WebsocketStrategy {
    type Message: for<'a> Deserialize<'a>;

    fn new() -> Self;

    async fn handle_message(
        &self,
        socket: &mut WebSocket,
        message: Self::Message,
        session_id: &str,
        channel_id: &str,
        state: &AppState,
    );

    async fn check_format(
        &self,
        socket: &mut WebSocket,
        message: Value,
        channel: String,
        session_id: &str,
        channel_id: &str,
        state: &AppState,
    ) {
        let payload = message.to_string();
        match serde_json::from_str::<Self::Message>(&payload) {
            Ok(parsed) => {
                self.handle_message(socket, parsed, session_id, channel_id, state)
                    .await
            }
            Err(_) => {
                let emitter = state.event_emitter.lock().await;
                emitter.emit_log("WARN", &format!("Schema mismatch on {}", channel));
            }
        }
    }
}
