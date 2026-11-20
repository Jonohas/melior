//! What a client's offer says about which of its media sections still send.

use std::collections::HashSet;
use std::io::Cursor;
use webrtc::rtp_transceiver::rtp_transceiver_direction::RTCRtpTransceiverDirection;
use webrtc::sdp::description::session::SessionDescription;

const DIRECTIONS: [&str; 4] = ["sendrecv", "sendonly", "recvonly", "inactive"];

/// The mids of the media sections of `sdp` in which the client no longer sends:
/// rejected (port 0), or with a direction that has no `send` (`recvonly`,
/// `inactive`). A section without a direction attribute is `sendrecv`.
///
/// An SDP that does not parse yields no sections: the connection rejects it on
/// its own, and nothing may end because of a description nobody could read.
pub(super) fn sections_not_sending(sdp: &str) -> HashSet<String> {
    let Ok(description) = SessionDescription::unmarshal(&mut Cursor::new(sdp)) else {
        return HashSet::new();
    };
    let session_direction = direction_of(|key| description.has_attribute(key));
    description
        .media_descriptions
        .iter()
        .filter_map(|media| {
            let mid = media.attribute("mid").flatten()?;
            let direction = direction_of(|key| media.has_attribute(key)).or(session_direction);
            let rejected = media.media_name.port.value == 0;
            let sends = direction.is_none_or(|direction| direction.has_send());
            (rejected || !sends).then(|| mid.to_owned())
        })
        .collect()
}

fn direction_of(has: impl Fn(&str) -> bool) -> Option<RTCRtpTransceiverDirection> {
    DIRECTIONS.into_iter().find(|key| has(key)).map(RTCRtpTransceiverDirection::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write;

    fn offer(sections: &[(&str, u16, Option<&str>)]) -> String {
        let mut sdp = String::from("v=0\r\no=- 1 2 IN IP4 127.0.0.1\r\ns=-\r\nt=0 0\r\n");
        for (mid, port, direction) in sections {
            let _ = write!(
                sdp,
                "m=video {port} UDP/TLS/RTP/SAVPF 96\r\nc=IN IP4 0.0.0.0\r\na=mid:{mid}\r\na=rtpmap:96 VP8/90000\r\n"
            );
            if let Some(direction) = direction {
                let _ = write!(sdp, "a={direction}\r\n");
            }
        }
        sdp
    }

    fn mids(sdp: &str) -> Vec<String> {
        let mut mids: Vec<String> = sections_not_sending(sdp).into_iter().collect();
        mids.sort();
        mids
    }

    #[test]
    fn sections_that_send_are_not_listed() {
        let sdp = offer(&[("0", 9, Some("sendrecv")), ("1", 9, Some("sendonly")), ("2", 9, None)]);
        assert!(mids(&sdp).is_empty());
    }

    #[test]
    fn recvonly_and_inactive_sections_are_listed() {
        let sdp = offer(&[
            ("0", 9, Some("sendrecv")),
            ("1", 9, Some("recvonly")),
            ("2", 9, Some("inactive")),
        ]);
        assert_eq!(mids(&sdp), ["1", "2"]);
    }

    #[test]
    fn a_rejected_section_is_listed() {
        let sdp = offer(&[("0", 0, Some("sendrecv")), ("1", 9, Some("sendrecv"))]);
        assert_eq!(mids(&sdp), ["0"]);
    }

    #[test]
    fn an_unreadable_description_lists_nothing() {
        assert!(sections_not_sending("not sdp").is_empty());
    }
}
