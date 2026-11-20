//! The wire format: JSON objects of the form `{ "channel": ..., "payload": ... }`.
//!
//! These types exist only to be (de)serialized; they are translated to and from the
//! application's vocabulary at the edge so the wire format can evolve independently.

use melior_application::Signal;
#[cfg(test)]
use melior_application::TrackOwner;
use melior_domain::{ChatMessage, IceCandidate, IceServer, ParticipantInfo, Sdp, TrackSource};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "channel", content = "payload", rename_all = "snake_case")]
pub enum ClientMessage {
    SdpOffer(SessionDescriptionDto),
    SdpAnswer(SessionDescriptionDto),
    Ice(IceCandidateDto),
    Echo(String),
    ChatSend(ChatSendDto),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "channel", content = "payload", rename_all = "snake_case")]
pub enum ServerMessage {
    Session {
        session_id: String,
        channel_id: String,
        username: String,
        ice_servers: Vec<IceServerDto>,
        participants: Vec<ParticipantDto>,
    },
    PeerJoined(ParticipantDto),
    SdpAnswer(SessionDescriptionDto),
    SdpOffer(OfferDto),
    Ice(IceCandidateDto),
    PeerLeft {
        session_id: String,
    },
    Error {
        message: String,
    },
    /// The last message before the server closes the socket on its own initiative.
    Closed {
        reason: CloseReason,
    },
    Echo(String),
    Chat(ChatMessageDto),
    /// A message the server did not accept. `client_id` names it when the client gave one.
    ChatError {
        #[serde(skip_serializing_if = "Option::is_none")]
        client_id: Option<String>,
        message: String,
    },
}

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

/// Why the server ends a connection that the client did not close.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CloseReason {
    /// The login session is over; log in again.
    SessionEnded,
    /// The user may no longer be in this room.
    Removed,
    /// Another server instance hosts this room now; reconnect.
    RoomMoved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SdpType {
    Offer,
    Answer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionDescriptionDto {
    #[serde(rename = "type")]
    pub kind: SdpType,
    pub sdp: String,
}

impl SessionDescriptionDto {
    /// The description's body, provided it is of the expected type.
    #[must_use]
    pub fn into_sdp(self, expected: SdpType) -> Option<Sdp> {
        (self.kind == expected).then(|| Sdp::new(self.sdp))
    }
}

/// A server offer: the description plus which media section carries whose track.
/// The list is complete each time; a section that is not in it carries nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OfferDto {
    #[serde(rename = "type")]
    pub kind: SdpType,
    pub sdp: String,
    pub tracks: Vec<TrackOwnerDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TrackOwnerDto {
    pub mid: String,
    pub session_id: String,
    /// What the track shows.
    pub source: TrackSourceDto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackSourceDto {
    Camera,
    Screen,
}

impl From<TrackSource> for TrackSourceDto {
    fn from(source: TrackSource) -> Self {
        match source {
            TrackSource::Camera => Self::Camera,
            TrackSource::Screen => Self::Screen,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ParticipantDto {
    pub session_id: String,
    pub username: String,
}

impl From<ParticipantInfo> for ParticipantDto {
    fn from(participant: ParticipantInfo) -> Self {
        Self { session_id: participant.session_id.to_string(), username: participant.username }
    }
}

/// Mirrors the browser's `RTCIceCandidateInit`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IceCandidateDto {
    pub candidate: String,
    #[serde(default)]
    pub sdp_mid: Option<String>,
    #[serde(default, rename = "sdpMLineIndex")]
    pub sdp_mline_index: Option<u16>,
    #[serde(default)]
    pub username_fragment: Option<String>,
}

/// Mirrors the browser's `RTCIceServer`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IceServerDto {
    pub urls: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential: Option<String>,
}

impl From<IceCandidateDto> for IceCandidate {
    fn from(dto: IceCandidateDto) -> Self {
        Self {
            candidate: dto.candidate,
            sdp_mid: dto.sdp_mid,
            sdp_mline_index: dto.sdp_mline_index,
            username_fragment: dto.username_fragment,
        }
    }
}

impl From<IceCandidate> for IceCandidateDto {
    fn from(candidate: IceCandidate) -> Self {
        Self {
            candidate: candidate.candidate,
            sdp_mid: candidate.sdp_mid,
            sdp_mline_index: candidate.sdp_mline_index,
            username_fragment: candidate.username_fragment,
        }
    }
}

impl From<IceServer> for IceServerDto {
    fn from(server: IceServer) -> Self {
        Self { urls: server.urls, username: server.username, credential: server.credential }
    }
}

impl From<Signal> for ServerMessage {
    fn from(signal: Signal) -> Self {
        match signal {
            Signal::Joined { session_id, channel_id, username, ice_servers, participants } => {
                Self::Session {
                    session_id: session_id.to_string(),
                    channel_id: channel_id.to_string(),
                    username,
                    ice_servers: ice_servers.into_iter().map(Into::into).collect(),
                    participants: participants.into_iter().map(Into::into).collect(),
                }
            }
            Signal::PeerJoined(participant) => Self::PeerJoined(participant.into()),
            Signal::Answer(sdp) => Self::SdpAnswer(SessionDescriptionDto {
                kind: SdpType::Answer,
                sdp: sdp.into_string(),
            }),
            Signal::Offer { sdp, tracks } => Self::SdpOffer(OfferDto {
                kind: SdpType::Offer,
                sdp: sdp.into_string(),
                tracks: tracks
                    .into_iter()
                    .map(|owner| TrackOwnerDto {
                        mid: owner.mid,
                        session_id: owner.session_id.to_string(),
                        source: owner.source.into(),
                    })
                    .collect(),
            }),
            Signal::IceCandidate(candidate) => Self::Ice(candidate.into()),
            Signal::PeerLeft(session_id) => Self::PeerLeft { session_id: session_id.to_string() },
            Signal::Chat { message, client_id } => {
                Self::Chat(ChatMessageDto::new(message, client_id))
            }
            Signal::Error(message) => Self::Error { message },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use melior_domain::{ChannelId, SessionId};
    use serde_json::json;

    #[test]
    fn parses_client_messages() {
        let offer: ClientMessage = serde_json::from_value(json!({
            "channel": "sdp_offer",
            "payload": { "type": "offer", "sdp": "v=0" }
        }))
        .unwrap();
        assert_eq!(
            offer,
            ClientMessage::SdpOffer(SessionDescriptionDto {
                kind: SdpType::Offer,
                sdp: "v=0".into()
            })
        );

        // What a browser's RTCIceCandidate.toJSON() produces.
        let ice: ClientMessage = serde_json::from_value(json!({
            "channel": "ice",
            "payload": {
                "candidate": "candidate:1 1 udp 1 127.0.0.1 9 typ host",
                "sdpMid": "0",
                "sdpMLineIndex": 0,
                "usernameFragment": null
            }
        }))
        .unwrap();
        assert_eq!(
            ice,
            ClientMessage::Ice(IceCandidateDto {
                candidate: "candidate:1 1 udp 1 127.0.0.1 9 typ host".into(),
                sdp_mid: Some("0".into()),
                sdp_mline_index: Some(0),
                username_fragment: None,
            })
        );

        let echo: ClientMessage =
            serde_json::from_value(json!({ "channel": "echo", "payload": "hi" })).unwrap();
        assert_eq!(echo, ClientMessage::Echo("hi".into()));
    }

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

    #[test]
    fn rejects_unknown_channels_and_bad_payloads() {
        for bad in [
            json!({ "channel": "nope", "payload": {} }),
            json!({ "channel": "sdp_offer", "payload": { "type": "pranswer", "sdp": "" } }),
            json!({ "channel": "ice", "payload": "not an object" }),
            json!({ "payload": {} }),
        ] {
            assert!(serde_json::from_value::<ClientMessage>(bad).is_err());
        }
    }

    #[test]
    fn description_type_must_match_the_channel() {
        let dto = SessionDescriptionDto { kind: SdpType::Answer, sdp: "v=0".into() };
        assert_eq!(dto.clone().into_sdp(SdpType::Offer), None);
        assert_eq!(dto.into_sdp(SdpType::Answer), Some(Sdp::new("v=0")));
    }

    #[test]
    fn serializes_signals_in_the_wire_format() {
        let session_id = SessionId::generate();
        let joined = ServerMessage::from(Signal::Joined {
            session_id: session_id.clone(),
            channel_id: ChannelId::parse("general").unwrap(),
            username: "bob".into(),
            participants: vec![ParticipantInfo {
                session_id: session_id.clone(),
                username: "alice".into(),
            }],
            ice_servers: vec![IceServer {
                urls: vec!["turn:localhost:3478".into()],
                username: Some("user".into()),
                credential: None,
            }],
        });
        assert_eq!(
            serde_json::to_value(joined).unwrap(),
            json!({
                "channel": "session",
                "payload": {
                    "session_id": session_id.as_str(),
                    "channel_id": "general",
                    "username": "bob",
                    "ice_servers": [{ "urls": ["turn:localhost:3478"], "username": "user" }],
                    "participants": [{ "session_id": session_id.as_str(), "username": "alice" }]
                }
            })
        );

        assert_eq!(
            serde_json::to_value(ServerMessage::from(Signal::PeerJoined(ParticipantInfo {
                session_id: session_id.clone(),
                username: "carol".into(),
            })))
            .unwrap(),
            json!({ "channel": "peer_joined", "payload": { "session_id": session_id.as_str(), "username": "carol" } })
        );

        assert_eq!(
            serde_json::to_value(ServerMessage::from(Signal::Offer {
                sdp: Sdp::new("v=0"),
                tracks: vec![TrackOwner {
                    mid: "1".into(),
                    session_id: session_id.clone(),
                    source: TrackSource::Screen
                },],
            }))
            .unwrap(),
            json!({
                "channel": "sdp_offer",
                "payload": {
                    "type": "offer",
                    "sdp": "v=0",
                    "tracks": [{ "mid": "1", "session_id": session_id.as_str(), "source": "screen" }]
                }
            })
        );

        assert_eq!(
            serde_json::to_value(ServerMessage::from(Signal::IceCandidate(IceCandidate {
                candidate: "c".into(),
                sdp_mid: Some("0".into()),
                sdp_mline_index: Some(0),
                username_fragment: None,
            })))
            .unwrap(),
            json!({
                "channel": "ice",
                "payload": { "candidate": "c", "sdpMid": "0", "sdpMLineIndex": 0, "usernameFragment": null }
            })
        );

        assert_eq!(
            serde_json::to_value(ServerMessage::from(Signal::PeerLeft(session_id.clone())))
                .unwrap(),
            json!({ "channel": "peer_left", "payload": { "session_id": session_id.as_str() } })
        );
    }

    #[test]
    fn serializes_the_close_reasons() {
        for (reason, name) in [
            (CloseReason::SessionEnded, "session_ended"),
            (CloseReason::Removed, "removed"),
            (CloseReason::RoomMoved, "room_moved"),
        ] {
            assert_eq!(
                serde_json::to_value(ServerMessage::Closed { reason }).unwrap(),
                json!({ "channel": "closed", "payload": { "reason": name } })
            );
        }
    }
}
