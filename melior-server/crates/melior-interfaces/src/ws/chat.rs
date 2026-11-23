//! The chat messages of the wire format: what a client says to the room and how a
//! saved message is shown, live and in the history.

use melior_application::ChatDraft;
use melior_domain::{
    Attachment, AttachmentId, AttachmentRequest, ChatContent, ChatError, ChatMessage, GifLink,
};
use serde::{Deserialize, Serialize};

/// What a client says to the room. `client_id` is the client's own name for the
/// message; the author's copy of it comes back carrying the same one. `text`
/// may be empty when there are attachments: 1 to 10 images, or one GIF.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ChatSendDto {
    #[serde(default)]
    pub client_id: Option<String>,
    pub text: String,
    #[serde(default)]
    pub attachments: Vec<AttachmentInputDto>,
}

impl ChatSendDto {
    /// The draft for the chat service, with the attachments parsed here at the
    /// edge. More than a message may carry is refused before any is parsed.
    pub fn into_draft(
        text: String,
        attachments: Vec<AttachmentInputDto>,
    ) -> Result<ChatDraft, ChatError> {
        if attachments.len() > ChatContent::MAX_IMAGES {
            return Err(ChatError::TooManyImages);
        }
        let attachments = attachments
            .into_iter()
            .map(AttachmentInputDto::into_request)
            .collect::<Result<Vec<_>, ChatError>>()?;
        Ok(ChatDraft { text, attachments })
    }
}

/// What a client attaches: an image it uploaded, or a GIF from KLIPY.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AttachmentInputDto {
    Image { id: String },
    Gif { url: String, width: u32, height: u32 },
}

impl AttachmentInputDto {
    fn into_request(self) -> Result<AttachmentRequest, ChatError> {
        match self {
            Self::Image { id } => AttachmentId::parse(&id)
                .map(AttachmentRequest::Image)
                .ok_or(ChatError::UnknownImage),
            Self::Gif { url, width, height } => {
                Ok(AttachmentRequest::Gif(GifLink::parse(&url, width, height)?))
            }
        }
    }
}

/// A saved chat message, as it is sent to the room and in the history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChatMessageDto {
    pub id: i64,
    pub username: String,
    /// `""` for a message that has only an attachment.
    pub text: String,
    /// Milliseconds since the Unix epoch.
    pub sent_at_ms: i64,
    /// Set only on the author's own copy of a message it just sent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    /// In the order they were sent; left out when there are none.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<AttachmentDto>,
}

/// A saved message's attachment. An image is fetched from
/// `GET /api/attachments/{id}`; a GIF straight from KLIPY.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AttachmentDto {
    Image { id: String, content_type: &'static str },
    Gif { url: String, width: u16, height: u16 },
}

impl From<&Attachment> for AttachmentDto {
    fn from(attachment: &Attachment) -> Self {
        match attachment {
            Attachment::Image { id, format } => {
                Self::Image { id: id.to_string(), content_type: format.content_type() }
            }
            Attachment::Gif(gif) => {
                Self::Gif { url: gif.url().to_owned(), width: gif.width(), height: gif.height() }
            }
        }
    }
}

impl ChatMessageDto {
    #[must_use]
    pub fn new(message: ChatMessage, client_id: Option<String>) -> Self {
        Self {
            id: message.id.value(),
            username: message.author,
            text: message.content.text_str().to_owned(),
            sent_at_ms: message.sent_at_ms,
            client_id,
            attachments: message.content.attachments().iter().map(AttachmentDto::from).collect(),
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
                text: "hi".into(),
                attachments: Vec::new()
            })
        );

        let without: ClientMessage =
            serde_json::from_value(json!({ "channel": "chat_send", "payload": { "text": "hi" } }))
                .unwrap();
        assert_eq!(
            without,
            ClientMessage::ChatSend(ChatSendDto {
                client_id: None,
                text: "hi".into(),
                attachments: Vec::new()
            })
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
            content: melior_domain::ChatText::parse("hello").unwrap().into(),
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

    #[test]
    fn attachments_are_parsed_at_the_edge() {
        let send = |attachments: serde_json::Value| {
            let message: ClientMessage = serde_json::from_value(json!({
                "channel": "chat_send", "payload": { "text": "", "attachments": attachments }
            }))
            .unwrap();
            let ClientMessage::ChatSend(dto) = message else { panic!("not a chat message") };
            ChatSendDto::into_draft(dto.text, dto.attachments)
        };
        let (a, b) = (AttachmentId::generate(), AttachmentId::generate());
        let two = json!([{ "kind": "image", "id": a.to_string() }, { "kind": "image", "id": b.to_string() }]);
        assert_eq!(
            send(two).unwrap().attachments,
            [AttachmentRequest::Image(a), AttachmentRequest::Image(b)]
        );
        let gif = send(
            json!([{ "kind": "gif", "url": "https://static.klipy.com/a.gif", "width": 2, "height": 3 }]),
        );
        assert!(matches!(gif.unwrap().attachments[..], [AttachmentRequest::Gif(_)]));
        assert_eq!(
            send(json!([{ "kind": "image", "id": "../etc" }])),
            Err(ChatError::UnknownImage)
        );
        assert_eq!(
            send(
                json!([{ "kind": "gif", "url": "https://evil.example/a.gif", "width": 2, "height": 3 }])
            ),
            Err(ChatError::InvalidGif)
        );
        let eleven: Vec<_> = (0..11).map(|_| json!({ "kind": "image", "id": "x" })).collect();
        assert_eq!(send(json!(eleven)), Err(ChatError::TooManyImages));
        let unknown = json!({ "channel": "chat_send", "payload": { "text": "", "attachments": [{ "kind": "file" }] } });
        assert!(serde_json::from_value::<ClientMessage>(unknown).is_err());
    }

    #[test]
    fn serializes_attachments() {
        let id = AttachmentId::generate();
        let gif = GifLink::parse("https://static.klipy.com/a.gif", 20, 10).unwrap();
        let message = |attachment| ChatMessage {
            id: melior_domain::MessageId::new(1),
            channel_id: ChannelId::parse("general").unwrap(),
            author: "alice".into(),
            content: ChatContent::new(None, vec![attachment]).unwrap(),
            sent_at_ms: 5,
        };
        let image = Attachment::Image { id: id.clone(), format: melior_domain::ImageFormat::Jpeg };
        assert_eq!(
            serde_json::to_value(ChatMessageDto::new(message(image), None)).unwrap(),
            json!({ "id": 1, "username": "alice", "text": "", "sent_at_ms": 5,
                    "attachments": [{ "kind": "image", "id": id.to_string(), "content_type": "image/jpeg" }] })
        );
        assert_eq!(
            serde_json::to_value(ChatMessageDto::new(message(Attachment::Gif(gif)), None)).unwrap(),
            json!({ "id": 1, "username": "alice", "text": "", "sent_at_ms": 5,
                    "attachments": [{ "kind": "gif", "url": "https://static.klipy.com/a.gif", "width": 20, "height": 10 }] })
        );
    }
}
