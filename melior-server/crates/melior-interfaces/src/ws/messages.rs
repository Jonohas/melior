//! The wire format: JSON objects of the form `{ "channel": ..., "payload": ... }`.
//!
//! These types exist only to be (de)serialized; they are translated to and from the
//! application's vocabulary at the edge so the wire format can evolve independently.

use super::peer::{LinkFailedDto, PeerDescriptionDto, PeerIceDto, RouteLiveDto};
use melior_application::Signal;
#[cfg(test)]
use melior_application::TrackOwner;
use melior_domain::{IceCandidate, IceServer, MediaPath, ParticipantInfo, Sdp};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "channel", content = "payload", rename_all = "snake_case")]
pub enum ClientMessage {
    SdpOffer(SessionDescriptionDto),
    SdpAnswer(SessionDescriptionDto),
    Ice(IceCandidateDto),
    RouteLive(RouteLiveDto),
    LinkFailed(LinkFailedDto),
    PeerOffer(PeerDescriptionDto),
    PeerAnswer(PeerDescriptionDto),
    PeerIce(PeerIceDto),
    Echo(String),
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
    /// Play `peer`'s media over `path` as soon as it works there, then report `route_live`.
    Route {
        peer: String,
        path: PathDto,
        epoch: u64,
    },
    /// Close the direct connection to `peer`.
    LinkClose {
        peer: String,
    },
    /// Pause (`false`) or resume (`true`) sending on the SFU connection.
    Uplink {
        active: bool,
    },
    /// A relayed offer for the direct connection with `from`.
    PeerOffer {
        from: String,
        sdp: String,
    },
    /// A relayed answer for the direct connection with `from`.
    PeerAnswer {
        from: String,
        sdp: String,
    },
    /// A relayed ICE candidate for the direct connection with `from`.
    PeerIce {
        from: String,
        candidate: IceCandidateDto,
    },
    Error {
        message: String,
    },
    /// The last message before the server closes the socket on its own initiative.
    Closed {
        reason: CloseReason,
    },
    Echo(String),
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

/// The path media takes between two participants, as clients name it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PathDto {
    Direct,
    Forwarded,
}

impl From<MediaPath> for PathDto {
    fn from(path: MediaPath) -> Self {
        match path {
            MediaPath::Direct => Self::Direct,
            MediaPath::Forwarded => Self::Forwarded,
        }
    }
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
                    })
                    .collect(),
            }),
            Signal::IceCandidate(candidate) => Self::Ice(candidate.into()),
            Signal::PeerLeft(session_id) => Self::PeerLeft { session_id: session_id.to_string() },
            Signal::Route { peer, path, epoch } => {
                Self::Route { peer: peer.to_string(), path: path.into(), epoch }
            }
            Signal::LinkClose { peer } => Self::LinkClose { peer: peer.to_string() },
            Signal::Uplink { active } => Self::Uplink { active },
            Signal::PeerOffer { from, sdp } => {
                Self::PeerOffer { from: from.to_string(), sdp: sdp.into_string() }
            }
            Signal::PeerAnswer { from, sdp } => {
                Self::PeerAnswer { from: from.to_string(), sdp: sdp.into_string() }
            }
            Signal::PeerIce { from, candidate } => {
                Self::PeerIce { from: from.to_string(), candidate: candidate.into() }
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
                tracks: vec![TrackOwner { mid: "1".into(), session_id: session_id.clone() }],
            }))
            .unwrap(),
            json!({
                "channel": "sdp_offer",
                "payload": {
                    "type": "offer",
                    "sdp": "v=0",
                    "tracks": [{ "mid": "1", "session_id": session_id.as_str() }]
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
    fn serializes_the_mesh_signals_in_the_wire_format() {
        let peer = SessionId::generate();
        let wire = |signal| serde_json::to_value(ServerMessage::from(signal)).unwrap();

        for (path, name) in [(MediaPath::Direct, "direct"), (MediaPath::Forwarded, "forwarded")] {
            assert_eq!(
                wire(Signal::Route { peer: peer.clone(), path, epoch: 7 }),
                json!({ "channel": "route", "payload": { "peer": peer.as_str(), "path": name, "epoch": 7 } })
            );
        }
        assert_eq!(
            wire(Signal::LinkClose { peer: peer.clone() }),
            json!({ "channel": "link_close", "payload": { "peer": peer.as_str() } })
        );
        assert_eq!(
            wire(Signal::Uplink { active: false }),
            json!({ "channel": "uplink", "payload": { "active": false } })
        );
        assert_eq!(
            wire(Signal::PeerOffer { from: peer.clone(), sdp: Sdp::new("v=0") }),
            json!({ "channel": "peer_offer", "payload": { "from": peer.as_str(), "sdp": "v=0" } })
        );
        assert_eq!(
            wire(Signal::PeerAnswer { from: peer.clone(), sdp: Sdp::new("v=0") }),
            json!({ "channel": "peer_answer", "payload": { "from": peer.as_str(), "sdp": "v=0" } })
        );
        assert_eq!(
            wire(Signal::PeerIce {
                from: peer.clone(),
                candidate: IceCandidate {
                    candidate: "c".into(),
                    sdp_mid: Some("0".into()),
                    sdp_mline_index: Some(1),
                    username_fragment: None,
                },
            }),
            json!({
                "channel": "peer_ice",
                "payload": {
                    "from": peer.as_str(),
                    "candidate": { "candidate": "c", "sdpMid": "0", "sdpMLineIndex": 1, "usernameFragment": null }
                }
            })
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
