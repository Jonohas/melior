//! How community events and voice entries are written to Redis. What is read back
//! comes from another instance (or anyone with access to Redis), so it is parsed
//! into domain types and refused when it does not fit.

use melior_application::{CommunityEvent, RemovalReason, VoiceEntry};
use melior_domain::{
    Attachment, AttachmentId, ChannelId, ChatContent, ChatMessage, ChatText, GifLink, ImageFormat,
    MessageId, SessionId, UserId, VoiceState,
};
use serde::{Deserialize, Serialize};

// The four flags of a voice state, as they are stored.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Serialize, Deserialize)]
pub(super) struct EntryRecord {
    channel_id: String,
    session_id: String,
    user_id: String,
    username: String,
    muted: bool,
    camera: bool,
    screen: bool,
    speaking: bool,
}

impl From<&VoiceEntry> for EntryRecord {
    fn from(entry: &VoiceEntry) -> Self {
        Self {
            channel_id: entry.channel_id.to_string(),
            session_id: entry.session_id.to_string(),
            user_id: entry.user_id.to_string(),
            username: entry.username.clone(),
            muted: entry.voice.muted,
            camera: entry.voice.camera,
            screen: entry.voice.screen,
            speaking: entry.voice.speaking,
        }
    }
}

impl EntryRecord {
    pub(super) fn into_entry(self) -> Option<VoiceEntry> {
        Some(VoiceEntry {
            channel_id: ChannelId::parse(&self.channel_id).ok()?,
            session_id: SessionId::parse(&self.session_id)?,
            user_id: UserId::new(self.user_id),
            username: self.username,
            voice: VoiceState {
                muted: self.muted,
                camera: self.camera,
                screen: self.screen,
                speaking: self.speaking,
            },
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct MessageRecord {
    id: i64,
    channel_id: String,
    author: String,
    text: String,
    sent_at_ms: i64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    attachments: Vec<AttachmentRecord>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(super) enum AttachmentRecord {
    Image { id: String, content_type: String },
    Gif { url: String, width: u32, height: u32 },
}

impl From<&ChatMessage> for MessageRecord {
    fn from(message: &ChatMessage) -> Self {
        let attachments = message.content.attachments().iter().map(|attachment| match attachment {
            Attachment::Image { id, format } => AttachmentRecord::Image {
                id: id.to_string(),
                content_type: format.content_type().to_owned(),
            },
            Attachment::Gif(gif) => AttachmentRecord::Gif {
                url: gif.url().to_owned(),
                width: u32::from(gif.width()),
                height: u32::from(gif.height()),
            },
        });
        let attachments = attachments.collect();
        Self {
            id: message.id.value(),
            channel_id: message.channel_id.to_string(),
            author: message.author.clone(),
            text: message.content.text_str().to_owned(),
            sent_at_ms: message.sent_at_ms,
            attachments,
        }
    }
}

impl MessageRecord {
    fn into_message(self) -> Option<ChatMessage> {
        let attachments = self
            .attachments
            .into_iter()
            .map(AttachmentRecord::into_attachment)
            .collect::<Option<Vec<_>>>()?;
        let text = ChatText::parse_optional(&self.text).ok()?;
        Some(ChatMessage {
            id: MessageId::new(self.id),
            channel_id: ChannelId::parse(&self.channel_id).ok()?,
            author: self.author,
            content: ChatContent::new(text, attachments).ok()?,
            sent_at_ms: self.sent_at_ms,
        })
    }
}

impl AttachmentRecord {
    fn into_attachment(self) -> Option<Attachment> {
        Some(match self {
            Self::Image { id, content_type } => Attachment::Image {
                id: AttachmentId::parse(&id)?,
                format: ImageFormat::from_content_type(&content_type)?,
            },
            Self::Gif { url, width, height } => {
                Attachment::Gif(GifLink::parse(&url, width, height).ok()?)
            }
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum EventRecord {
    VoiceJoined { entry: EntryRecord },
    VoiceUpdated { entry: EntryRecord },
    VoiceLeft { channel_id: String, session_id: String },
    Message { message: MessageRecord },
    StructureChanged,
    MemberJoined { user_id: String },
    MemberRemoved { user_id: String, kicked: bool },
    CommunityDeleted,
}

impl From<&CommunityEvent> for EventRecord {
    fn from(event: &CommunityEvent) -> Self {
        match event {
            CommunityEvent::VoiceJoined(entry) => Self::VoiceJoined { entry: entry.into() },
            CommunityEvent::VoiceUpdated(entry) => Self::VoiceUpdated { entry: entry.into() },
            CommunityEvent::VoiceLeft { channel_id, session_id } => Self::VoiceLeft {
                channel_id: channel_id.to_string(),
                session_id: session_id.to_string(),
            },
            CommunityEvent::Message { message, .. } => Self::Message { message: message.into() },
            CommunityEvent::StructureChanged => Self::StructureChanged,
            CommunityEvent::MemberJoined { user_id } => {
                Self::MemberJoined { user_id: user_id.to_string() }
            }
            CommunityEvent::MemberRemoved { user_id, reason } => Self::MemberRemoved {
                user_id: user_id.to_string(),
                kicked: *reason == RemovalReason::Kicked,
            },
            CommunityEvent::CommunityDeleted => Self::CommunityDeleted,
        }
    }
}

impl EventRecord {
    pub(super) fn into_event(self) -> Option<CommunityEvent> {
        Some(match self {
            Self::VoiceJoined { entry } => CommunityEvent::VoiceJoined(entry.into_entry()?),
            Self::VoiceUpdated { entry } => CommunityEvent::VoiceUpdated(entry.into_entry()?),
            Self::VoiceLeft { channel_id, session_id } => CommunityEvent::VoiceLeft {
                channel_id: ChannelId::parse(&channel_id).ok()?,
                session_id: SessionId::parse(&session_id)?,
            },
            Self::Message { message } => {
                let message = message.into_message()?;
                CommunityEvent::Message { channel_id: message.channel_id.clone(), message }
            }
            Self::StructureChanged => CommunityEvent::StructureChanged,
            Self::MemberJoined { user_id } => {
                CommunityEvent::MemberJoined { user_id: UserId::new(user_id) }
            }
            Self::MemberRemoved { user_id, kicked } => CommunityEvent::MemberRemoved {
                user_id: UserId::new(user_id),
                reason: if kicked { RemovalReason::Kicked } else { RemovalReason::Left },
            },
            Self::CommunityDeleted => CommunityEvent::CommunityDeleted,
        })
    }
}

/// An event as JSON.
pub(super) fn encode(event: &CommunityEvent) -> Result<String, serde_json::Error> {
    serde_json::to_string(&EventRecord::from(event))
}

/// An event from JSON; `None` for anything that does not fit.
pub(super) fn decode(json: &str) -> Option<CommunityEvent> {
    serde_json::from_str::<EventRecord>(json).ok()?.into_event()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry() -> VoiceEntry {
        VoiceEntry {
            channel_id: ChannelId::parse("general").unwrap(),
            session_id: SessionId::generate(),
            user_id: UserId::new("sub-alice"),
            username: "alice".into(),
            voice: VoiceState { muted: true, camera: false, screen: true, speaking: false },
        }
    }

    #[test]
    fn every_event_survives_the_round_trip() {
        let message = ChatMessage {
            id: MessageId::new(3),
            channel_id: ChannelId::parse("text").unwrap(),
            author: "bob".into(),
            content: ChatText::parse("hi").unwrap().into(),
            sent_at_ms: 17,
        };
        let gif = GifLink::parse("https://static.klipy.com/a.gif", 20, 10).unwrap();
        let with_gif = ChatMessage {
            content: ChatContent::new(None, vec![Attachment::Gif(gif)]).unwrap(),
            ..message.clone()
        };
        let image =
            || Attachment::Image { id: AttachmentId::generate(), format: ImageFormat::Webp };
        let with_image = ChatMessage {
            content: ChatContent::new(ChatText::parse("look").ok(), vec![image(), image()])
                .unwrap(),
            ..message.clone()
        };
        for event in [
            CommunityEvent::VoiceJoined(entry()),
            CommunityEvent::VoiceUpdated(entry()),
            CommunityEvent::VoiceLeft {
                channel_id: ChannelId::parse("general").unwrap(),
                session_id: SessionId::generate(),
            },
            CommunityEvent::Message { channel_id: message.channel_id.clone(), message },
            CommunityEvent::Message { channel_id: with_gif.channel_id.clone(), message: with_gif },
            CommunityEvent::Message {
                channel_id: with_image.channel_id.clone(),
                message: with_image,
            },
            CommunityEvent::StructureChanged,
            CommunityEvent::MemberJoined { user_id: UserId::new("u") },
            CommunityEvent::MemberRemoved {
                user_id: UserId::new("u"),
                reason: RemovalReason::Kicked,
            },
            CommunityEvent::MemberRemoved {
                user_id: UserId::new("u"),
                reason: RemovalReason::Left,
            },
            CommunityEvent::CommunityDeleted,
        ] {
            assert_eq!(decode(&encode(&event).unwrap()), Some(event));
        }
    }

    #[test]
    fn anything_that_does_not_fit_is_refused() {
        for bad in [
            "",
            "{}",
            r#"{"type":"nope"}"#,
            r#"{"type":"voice_left","channel_id":"bad id","session_id":"x"}"#,
            r#"{"type":"message","message":{"id":1,"channel_id":"c","author":"a","text":"  ","sent_at_ms":0}}"#,
            r#"{"type":"message","message":{"id":1,"channel_id":"c","author":"a","text":"","sent_at_ms":0,"attachments":[{"kind":"gif","url":"https://evil.example/a.gif","width":1,"height":1}]}}"#,
        ] {
            assert_eq!(decode(bad), None, "{bad:?}");
        }
    }
}
