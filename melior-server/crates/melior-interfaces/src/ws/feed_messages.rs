//! The community feed's wire format: `{ "channel": ..., "payload": ... }`, server
//! to client only.

use super::chat::ChatMessageDto;
use super::messages::{CloseReason, VoiceDto};
use melior_application::{CommunityEvent, FeedEnd, VoiceEntry};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "channel", content = "payload", rename_all = "snake_case")]
pub enum FeedMessage {
    /// Who is in which visible voice channel: sent first, and again after every
    /// re-check, so entries of a crashed instance and missed events heal.
    Snapshot {
        voice: Vec<VoiceEntryDto>,
    },
    VoiceJoined(VoiceEntryDto),
    VoiceUpdated(VoiceEntryDto),
    VoiceLeft {
        channel_id: String,
        session_id: String,
    },
    /// A message in a text channel.
    Message {
        channel_id: String,
        message: ChatMessageDto,
    },
    /// Refetch the community.
    StructureChanged {},
    /// Someone joined, left or was kicked: refetch the member list.
    MembersChanged {},
    /// The member is out of the community; the server closes the feed next.
    Removed {
        reason: RemovedReason,
    },
    /// The login session ended; the server closes the feed next.
    Closed {
        reason: CloseReason,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RemovedReason {
    Kicked,
    Left,
    Deleted,
}

impl From<FeedEnd> for RemovedReason {
    fn from(end: FeedEnd) -> Self {
        match end {
            FeedEnd::Kicked => Self::Kicked,
            FeedEnd::Left => Self::Left,
            FeedEnd::Deleted => Self::Deleted,
        }
    }
}

/// One participant of a voice channel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VoiceEntryDto {
    pub channel_id: String,
    pub session_id: String,
    pub user_id: String,
    pub username: String,
    #[serde(flatten)]
    pub voice: VoiceDto,
}

impl From<VoiceEntry> for VoiceEntryDto {
    fn from(entry: VoiceEntry) -> Self {
        Self {
            channel_id: entry.channel_id.to_string(),
            session_id: entry.session_id.to_string(),
            user_id: entry.user_id.to_string(),
            username: entry.username,
            voice: entry.voice.into(),
        }
    }
}

impl FeedMessage {
    /// The message for an event the feed forwards. Membership events are not
    /// forwarded as such: they end the feed or are dropped (see `FeedFilter`).
    #[must_use]
    pub fn forwarded(event: CommunityEvent) -> Option<Self> {
        Some(match event {
            CommunityEvent::VoiceJoined(entry) => Self::VoiceJoined(entry.into()),
            CommunityEvent::VoiceUpdated(entry) => Self::VoiceUpdated(entry.into()),
            CommunityEvent::VoiceLeft { channel_id, session_id } => Self::VoiceLeft {
                channel_id: channel_id.to_string(),
                session_id: session_id.to_string(),
            },
            CommunityEvent::Message { channel_id, message } => Self::Message {
                channel_id: channel_id.to_string(),
                message: ChatMessageDto::new(message, None),
            },
            CommunityEvent::StructureChanged => Self::StructureChanged {},
            CommunityEvent::MemberJoined { .. } | CommunityEvent::MemberRemoved { .. } => {
                Self::MembersChanged {}
            }
            CommunityEvent::CommunityDeleted => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use melior_domain::{
        ChannelId, ChatMessage, ChatText, MessageId, SessionId, UserId, VoiceState,
    };
    use serde_json::json;

    fn entry() -> VoiceEntry {
        VoiceEntry {
            channel_id: ChannelId::parse("general").unwrap(),
            session_id: SessionId::generate(),
            user_id: UserId::new("sub-alice"),
            username: "alice".into(),
            voice: VoiceState { muted: true, camera: true, screen: false, speaking: false },
        }
    }

    fn wire(message: FeedMessage) -> serde_json::Value {
        serde_json::to_value(message).unwrap()
    }

    #[test]
    fn voice_entries_carry_the_four_flags_flat() {
        let entry = entry();
        let session = entry.session_id.to_string();
        let flat = json!({
            "channel_id": "general", "session_id": session, "user_id": "sub-alice", "username": "alice",
            "muted": true, "camera": true, "screen": false, "speaking": false
        });
        assert_eq!(
            wire(FeedMessage::Snapshot { voice: vec![entry.clone().into()] }),
            json!({ "channel": "snapshot", "payload": { "voice": [flat.clone()] } })
        );
        assert_eq!(
            wire(FeedMessage::forwarded(CommunityEvent::VoiceJoined(entry.clone())).unwrap()),
            json!({ "channel": "voice_joined", "payload": flat.clone() })
        );
        assert_eq!(
            wire(FeedMessage::forwarded(CommunityEvent::VoiceUpdated(entry.clone())).unwrap()),
            json!({ "channel": "voice_updated", "payload": flat })
        );
        let left = CommunityEvent::VoiceLeft {
            channel_id: entry.channel_id.clone(),
            session_id: entry.session_id.clone(),
        };
        assert_eq!(
            wire(FeedMessage::forwarded(left).unwrap()),
            json!({ "channel": "voice_left", "payload": { "channel_id": "general", "session_id": session } })
        );
    }

    #[test]
    fn messages_structure_changes_removals_and_closes() {
        let message = ChatMessage {
            id: MessageId::new(4),
            channel_id: ChannelId::parse("text").unwrap(),
            author: "bob".into(),
            content: ChatText::parse("hi").unwrap().into(),
            sent_at_ms: 9,
        };
        let event = CommunityEvent::Message { channel_id: message.channel_id.clone(), message };
        assert_eq!(
            wire(FeedMessage::forwarded(event).unwrap()),
            json!({ "channel": "message", "payload": {
                "channel_id": "text",
                "message": { "id": 4, "username": "bob", "text": "hi", "sent_at_ms": 9 }
            } })
        );
        assert_eq!(
            wire(FeedMessage::forwarded(CommunityEvent::StructureChanged).unwrap()),
            json!({ "channel": "structure_changed", "payload": {} })
        );
        for (reason, name) in [
            (RemovedReason::Kicked, "kicked"),
            (RemovedReason::Left, "left"),
            (RemovedReason::Deleted, "deleted"),
        ] {
            assert_eq!(
                wire(FeedMessage::Removed { reason }),
                json!({ "channel": "removed", "payload": { "reason": name } })
            );
        }
        assert_eq!(
            wire(FeedMessage::Closed { reason: CloseReason::SessionEnded }),
            json!({ "channel": "closed", "payload": { "reason": "session_ended" } })
        );
        let joined = CommunityEvent::MemberJoined { user_id: UserId::new("sub-dave") };
        assert_eq!(
            wire(FeedMessage::forwarded(joined).unwrap()),
            json!({ "channel": "members_changed", "payload": {} })
        );
        assert_eq!(FeedMessage::forwarded(CommunityEvent::CommunityDeleted), None);
    }
}
