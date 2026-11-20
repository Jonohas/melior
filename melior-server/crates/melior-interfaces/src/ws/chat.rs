//! The chat messages of the wire format: what a client says to the room and how a
//! saved message is shown, live and in the history.

use melior_domain::ChatMessage;
use serde::{Deserialize, Serialize};

/// What a client says to the room. `client_id` is the client's own name for the
/// message; the author's copy of it comes back carrying the same one.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ChatSendDto {
    #[serde(default)]
    pub client_id: Option<String>,
    pub text: String,
}

/// A saved chat message, as it is sent to the room and in the history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChatMessageDto {
    pub id: i64,
    pub username: String,
    pub text: String,
    /// Milliseconds since the Unix epoch.
    pub sent_at_ms: i64,
    /// Set only on the author's own copy of a message it just sent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
}

impl ChatMessageDto {
    #[must_use]
    pub fn new(message: ChatMessage, client_id: Option<String>) -> Self {
        Self {
            id: message.id.value(),
            username: message.author,
            text: message.text.to_string(),
            sent_at_ms: message.sent_at_ms,
            client_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::messages::{ClientMessage, ServerMessage};
    use super::*;
    use melior_application::Signal;
    use melior_domain::ChannelId;
    use serde_json::json;

    #[test]
    fn parses_a_chat_message_with_and_without_a_client_id() {
        let with: ClientMessage = serde_json::from_value(
            json!({ "channel": "chat_send", "payload": { "client_id": "c1", "text": "hi" } }),
        )
        .unwrap();
        assert_eq!(
            with,
            ClientMessage::ChatSend(ChatSendDto {
                client_id: Some("c1".into()),
                text: "hi".into()
            })
        );

        let without: ClientMessage =
            serde_json::from_value(json!({ "channel": "chat_send", "payload": { "text": "hi" } }))
                .unwrap();
        assert_eq!(
            without,
            ClientMessage::ChatSend(ChatSendDto { client_id: None, text: "hi".into() })
        );
    }

    #[test]
    fn a_chat_message_needs_a_text() {
        for bad in [
            json!({ "channel": "chat_send", "payload": {} }),
            json!({ "channel": "chat_send", "payload": { "text": 5 } }),
            json!({ "channel": "chat_send", "payload": "hi" }),
        ] {
            assert!(serde_json::from_value::<ClientMessage>(bad).is_err());
        }
    }

    #[test]
    fn serializes_chat_messages_and_refusals() {
        let message = ChatMessage {
            id: melior_domain::MessageId::new(7),
            channel_id: ChannelId::parse("general").unwrap(),
            author: "alice".into(),
            text: melior_domain::ChatText::parse("hello").unwrap(),
            sent_at_ms: 1_700_000_000_000,
        };

        assert_eq!(
            serde_json::to_value(ServerMessage::from(Signal::Chat {
                message: message.clone(),
                client_id: Some("c1".into()),
            }))
            .unwrap(),
            json!({
                "channel": "chat",
                "payload": { "id": 7, "username": "alice", "text": "hello", "sent_at_ms": 1_700_000_000_000_i64, "client_id": "c1" }
            })
        );
        // Everyone but the author sees no client id.
        assert_eq!(
            serde_json::to_value(ServerMessage::from(Signal::Chat { message, client_id: None }))
                .unwrap(),
            json!({
                "channel": "chat",
                "payload": { "id": 7, "username": "alice", "text": "hello", "sent_at_ms": 1_700_000_000_000_i64 }
            })
        );
        assert_eq!(
            serde_json::to_value(ServerMessage::ChatError {
                client_id: Some("c1".into()),
                message: "a message cannot be empty".into(),
            })
            .unwrap(),
            json!({ "channel": "chat_error", "payload": { "client_id": "c1", "message": "a message cannot be empty" } })
        );
    }
}
