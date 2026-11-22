//! The client-to-server messages of the mesh: path reports and the relay of direct
//! connection signalling. Ids and sizes are checked here, at the edge, so the
//! application only ever sees valid values.

use super::messages::IceCandidateDto;
use melior_domain::{IceCandidate, KeyEnvelope, Sdp, SessionId};
use serde::Deserialize;

/// The largest session description a client may relay.
pub const MAX_SDP_BYTES: usize = 64 * 1024;
/// The longest ICE candidate string a client may send.
pub const MAX_CANDIDATE_BYTES: usize = 1024;
/// The longest `sdpMid` a client may send with an ICE candidate.
pub const MAX_SDP_MID_BYTES: usize = 256;
/// The longest `usernameFragment` a client may send with an ICE candidate.
pub const MAX_USERNAME_FRAGMENT_BYTES: usize = 256;

/// `{ "peer": <session id>, "epoch": <u64> }`: I now play `peer`'s media over the
/// path of route `epoch`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RouteLiveDto {
    pub peer: String,
    pub epoch: u64,
}

/// `{ "peer": <session id> }`: my direct link to `peer` failed.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LinkFailedDto {
    pub peer: String,
}

/// `{ "to": <session id>, "sdp": <string> }`: an offer or answer for the direct link.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PeerDescriptionDto {
    pub to: String,
    pub sdp: String,
}

/// `{ "to": <session id>, "candidate": <RTCIceCandidateInit> }`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PeerIceDto {
    pub to: String,
    pub candidate: IceCandidateDto,
}

/// `{ "to": <session id>, "payload": <base64url> }`: an end-to-end key message for
/// another participant of the room, relayed unread.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct KeyDto {
    pub to: String,
    pub payload: String,
}

/// A mesh message that passed the edge checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeerCommand {
    RouteLive { peer: SessionId, epoch: u64 },
    LinkFailed { peer: SessionId },
    Offer { to: SessionId, sdp: Sdp },
    Answer { to: SessionId, sdp: Sdp },
    Ice { to: SessionId, candidate: IceCandidate },
    Key { to: SessionId, envelope: KeyEnvelope },
}

/// Why a mesh message is dropped. It is logged and never answered: a client tears
/// everything down on `error`, and none of these is worth that.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rejection {
    BadSessionId,
    SdpTooLarge,
    CandidateTooLarge,
    CandidateFieldTooLarge,
    /// A key message that is empty, too large or not base64url.
    BadKeyEnvelope,
}

fn session(raw: &str) -> Result<SessionId, Rejection> {
    SessionId::parse(raw).ok_or(Rejection::BadSessionId)
}

fn sdp(raw: String) -> Result<Sdp, Rejection> {
    if raw.len() > MAX_SDP_BYTES {
        return Err(Rejection::SdpTooLarge);
    }
    Ok(Sdp::new(raw))
}

impl RouteLiveDto {
    /// # Errors
    /// [`Rejection::BadSessionId`] when `peer` is not a session id.
    pub fn validate(self) -> Result<PeerCommand, Rejection> {
        Ok(PeerCommand::RouteLive { peer: session(&self.peer)?, epoch: self.epoch })
    }
}

impl LinkFailedDto {
    /// # Errors
    /// [`Rejection::BadSessionId`] when `peer` is not a session id.
    pub fn validate(self) -> Result<PeerCommand, Rejection> {
        Ok(PeerCommand::LinkFailed { peer: session(&self.peer)? })
    }
}

impl KeyDto {
    /// # Errors
    /// A bad target id, or a payload that is no valid [`KeyEnvelope`].
    pub fn validate(self) -> Result<PeerCommand, Rejection> {
        let envelope = KeyEnvelope::parse(&self.payload).ok_or(Rejection::BadKeyEnvelope)?;
        Ok(PeerCommand::Key { to: session(&self.to)?, envelope })
    }
}

impl PeerDescriptionDto {
    /// # Errors
    /// A bad target id or an oversized description.
    pub fn validate_offer(self) -> Result<PeerCommand, Rejection> {
        Ok(PeerCommand::Offer { to: session(&self.to)?, sdp: sdp(self.sdp)? })
    }

    /// # Errors
    /// A bad target id or an oversized description.
    pub fn validate_answer(self) -> Result<PeerCommand, Rejection> {
        Ok(PeerCommand::Answer { to: session(&self.to)?, sdp: sdp(self.sdp)? })
    }
}

impl IceCandidateDto {
    /// The candidate as the application takes it, provided no field is oversized.
    ///
    /// # Errors
    /// [`Rejection::CandidateTooLarge`] or [`Rejection::CandidateFieldTooLarge`].
    pub fn into_checked(self) -> Result<IceCandidate, Rejection> {
        if self.candidate.len() > MAX_CANDIDATE_BYTES {
            return Err(Rejection::CandidateTooLarge);
        }
        let too_long = |field: &Option<String>, max: usize| {
            field.as_ref().is_some_and(|text| text.len() > max)
        };
        if too_long(&self.sdp_mid, MAX_SDP_MID_BYTES)
            || too_long(&self.username_fragment, MAX_USERNAME_FRAGMENT_BYTES)
        {
            return Err(Rejection::CandidateFieldTooLarge);
        }
        Ok(self.into())
    }
}

impl PeerIceDto {
    /// # Errors
    /// A bad target id or an oversized candidate.
    pub fn validate(self) -> Result<PeerCommand, Rejection> {
        let to = session(&self.to)?;
        Ok(PeerCommand::Ice { to, candidate: self.candidate.into_checked()? })
    }
}

#[cfg(test)]
mod tests {
    use super::super::messages::ClientMessage;
    use super::*;
    use serde_json::json;

    fn parse(value: serde_json::Value) -> ClientMessage {
        serde_json::from_value(value).unwrap()
    }

    fn id() -> SessionId {
        SessionId::generate()
    }

    #[test]
    fn route_live_carries_peer_and_epoch() {
        let peer = id();
        let message = parse(json!({
            "channel": "route_live",
            "payload": { "peer": peer.as_str(), "epoch": 3 }
        }));
        let ClientMessage::RouteLive(dto) = message else { panic!("wrong variant") };
        assert_eq!(dto.validate(), Ok(PeerCommand::RouteLive { peer, epoch: 3 }));
    }

    #[test]
    fn link_failed_carries_the_peer() {
        let peer = id();
        let message =
            parse(json!({ "channel": "link_failed", "payload": { "peer": peer.as_str() } }));
        let ClientMessage::LinkFailed(dto) = message else { panic!("wrong variant") };
        assert_eq!(dto.validate(), Ok(PeerCommand::LinkFailed { peer }));
    }

    #[test]
    fn e2ee_carries_target_and_an_opaque_payload() {
        let to = id();
        let message =
            parse(json!({ "channel": "e2ee", "payload": { "to": to.as_str(), "payload": "aGk" } }));
        assert!(message.is_key() && !message.is_relay(), "key messages have a limit of their own");
        let ClientMessage::E2ee(dto) = message else { panic!("wrong variant") };
        assert_eq!(
            dto.validate(),
            Ok(PeerCommand::Key { to, envelope: KeyEnvelope::parse("aGk").unwrap() })
        );
    }

    #[test]
    fn e2ee_rejects_a_bad_payload_or_target() {
        let to = id();
        for payload in ["", "not base64!", &"a".repeat(melior_domain::MAX_KEY_ENVELOPE_BYTES + 1)] {
            let dto = KeyDto { to: to.as_str().to_owned(), payload: payload.to_owned() };
            assert_eq!(dto.validate(), Err(Rejection::BadKeyEnvelope));
        }
        let dto = KeyDto { to: "nope".into(), payload: "aGk".into() };
        assert_eq!(dto.validate(), Err(Rejection::BadSessionId));
    }

    #[test]
    fn peer_offer_and_answer_carry_target_and_sdp() {
        let to = id();
        let payload = json!({ "to": to.as_str(), "sdp": "v=0" });
        let ClientMessage::PeerOffer(offer) =
            parse(json!({ "channel": "peer_offer", "payload": payload }))
        else {
            panic!("wrong variant")
        };
        assert_eq!(
            offer.validate_offer(),
            Ok(PeerCommand::Offer { to: to.clone(), sdp: Sdp::new("v=0") })
        );
        let ClientMessage::PeerAnswer(answer) =
            parse(json!({ "channel": "peer_answer", "payload": payload }))
        else {
            panic!("wrong variant")
        };
        assert_eq!(answer.validate_answer(), Ok(PeerCommand::Answer { to, sdp: Sdp::new("v=0") }));
    }

    #[test]
    fn peer_ice_carries_target_and_candidate() {
        let to = id();
        let message = parse(json!({
            "channel": "peer_ice",
            "payload": {
                "to": to.as_str(),
                "candidate": { "candidate": "c", "sdpMid": "0", "sdpMLineIndex": 1, "usernameFragment": null }
            }
        }));
        let ClientMessage::PeerIce(dto) = message else { panic!("wrong variant") };
        assert_eq!(
            dto.validate(),
            Ok(PeerCommand::Ice {
                to,
                candidate: IceCandidate {
                    candidate: "c".into(),
                    sdp_mid: Some("0".into()),
                    sdp_mline_index: Some(1),
                    username_fragment: None,
                },
            })
        );
    }

    #[test]
    fn payloads_missing_fields_do_not_parse() {
        for bad in [
            json!({ "channel": "route_live", "payload": { "peer": "x" } }),
            json!({ "channel": "route_live", "payload": { "peer": "x", "epoch": -1 } }),
            json!({ "channel": "link_failed", "payload": {} }),
            json!({ "channel": "peer_offer", "payload": { "to": "x" } }),
            json!({ "channel": "peer_ice", "payload": { "to": "x", "candidate": "c" } }),
        ] {
            assert!(serde_json::from_value::<ClientMessage>(bad).is_err());
        }
    }

    #[test]
    fn an_id_that_is_not_a_session_id_is_rejected() {
        let bad = || "../etc/passwd".to_owned();
        assert_eq!(RouteLiveDto { peer: bad(), epoch: 1 }.validate(), Err(Rejection::BadSessionId));
        assert_eq!(LinkFailedDto { peer: bad() }.validate(), Err(Rejection::BadSessionId));
        let description = || PeerDescriptionDto { to: bad(), sdp: "v=0".into() };
        assert_eq!(description().validate_offer(), Err(Rejection::BadSessionId));
        assert_eq!(description().validate_answer(), Err(Rejection::BadSessionId));
        let ice = PeerIceDto {
            to: bad(),
            candidate: IceCandidateDto {
                candidate: "c".into(),
                sdp_mid: None,
                sdp_mline_index: None,
                username_fragment: None,
            },
        };
        assert_eq!(ice.validate(), Err(Rejection::BadSessionId));
    }

    #[test]
    fn an_sdp_is_limited_to_64_kib() {
        let to = id();
        let with = |len| PeerDescriptionDto { to: to.to_string(), sdp: "a".repeat(len) };
        assert!(with(MAX_SDP_BYTES).validate_offer().is_ok());
        assert_eq!(with(MAX_SDP_BYTES + 1).validate_offer(), Err(Rejection::SdpTooLarge));
        assert_eq!(with(MAX_SDP_BYTES + 1).validate_answer(), Err(Rejection::SdpTooLarge));
    }

    #[test]
    fn candidate_fields_are_limited() {
        let candidate =
            |sdp_mid: Option<String>, username_fragment: Option<String>| IceCandidateDto {
                candidate: "c".into(),
                sdp_mid,
                sdp_mline_index: None,
                username_fragment,
            };
        let long = |max: usize| Some("a".repeat(max));
        let over = |max: usize| Some("a".repeat(max + 1));
        assert!(
            candidate(long(MAX_SDP_MID_BYTES), long(MAX_USERNAME_FRAGMENT_BYTES))
                .into_checked()
                .is_ok()
        );
        assert_eq!(
            candidate(over(MAX_SDP_MID_BYTES), None).into_checked(),
            Err(Rejection::CandidateFieldTooLarge)
        );
        assert_eq!(
            candidate(None, over(MAX_USERNAME_FRAGMENT_BYTES)).into_checked(),
            Err(Rejection::CandidateFieldTooLarge)
        );
        let relayed = PeerIceDto {
            to: SessionId::generate().to_string(),
            candidate: candidate(over(MAX_SDP_MID_BYTES), None),
        };
        assert_eq!(relayed.validate(), Err(Rejection::CandidateFieldTooLarge));
    }

    #[test]
    fn a_candidate_is_limited_to_1_kib() {
        let to = id();
        let with = |len| PeerIceDto {
            to: to.to_string(),
            candidate: IceCandidateDto {
                candidate: "a".repeat(len),
                sdp_mid: None,
                sdp_mline_index: None,
                username_fragment: None,
            },
        };
        assert!(with(MAX_CANDIDATE_BYTES).validate().is_ok());
        assert_eq!(with(MAX_CANDIDATE_BYTES + 1).validate(), Err(Rejection::CandidateTooLarge));
    }
}
