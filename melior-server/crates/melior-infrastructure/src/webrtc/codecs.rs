//! Which codecs the SFU speaks, and how an audio track is sent to a participant
//! whose connection negotiated different ones than its publisher's.

use super::red;
use std::io::Cursor;
use webrtc::api::media_engine::{MIME_TYPE_OPUS, MediaEngine};
use webrtc::rtp::packet::Packet;
use webrtc::rtp_transceiver::rtp_codec::{
    RTCRtpCodecCapability, RTCRtpCodecParameters, RTPCodecType,
};
use webrtc::sdp::description::session::SessionDescription;

pub(super) const MIME_TYPE_RED: &str = "audio/red";
/// The numbers browsers use; a connection keeps whatever its client offered.
const OPUS_PAYLOAD_TYPE: u8 = 111;
const RED_PAYLOAD_TYPE: u8 = 63;
const OPUS_FMTP: &str = "minptime=10;useinbandfec=1";

fn opus() -> RTCRtpCodecCapability {
    RTCRtpCodecCapability {
        mime_type: MIME_TYPE_OPUS.to_owned(),
        clock_rate: 48_000,
        channels: 2,
        sdp_fmtp_line: OPUS_FMTP.to_owned(),
        rtcp_feedback: vec![],
    }
}

/// The webrtc-rs defaults plus RED over Opus. RED repeats the previous frame in
/// every packet: audio has no retransmission, and the publisher's Opus FEC only
/// reacts to the loss it hears about, which is its own uplink's, never a
/// subscriber's downlink.
pub(super) fn register(media_engine: &mut MediaEngine) -> Result<(), webrtc::Error> {
    let red = RTCRtpCodecParameters {
        capability: RTCRtpCodecCapability {
            mime_type: MIME_TYPE_RED.to_owned(),
            sdp_fmtp_line: format!("{OPUS_PAYLOAD_TYPE}/{OPUS_PAYLOAD_TYPE}"),
            ..opus()
        },
        payload_type: RED_PAYLOAD_TYPE,
        ..Default::default()
    };
    media_engine.register_codec(red, RTPCodecType::Audio)?;
    media_engine.register_default_codecs()
}

/// The audio a participant's connection negotiated, as far as forwarding cares.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct ReceiverAudio {
    /// How the participant numbers Opus, when it said.
    opus: Option<u8>,
    red: bool,
}

impl ReceiverAudio {
    /// Read from the participant's own description. webrtc-rs settles a
    /// connection's audio codecs on the first audio section it receives, so that
    /// section is the one that counts. A description that does not parse, or has
    /// no audio, means no RED.
    pub fn from_sdp(sdp: &str) -> Self {
        let Ok(description) = SessionDescription::unmarshal(&mut Cursor::new(sdp)) else {
            return Self::default();
        };
        let Some(audio) =
            description.media_descriptions.iter().find(|media| media.media_name.media == "audio")
        else {
            return Self::default();
        };
        let mut found = Self::default();
        for (payload_type, name) in audio
            .attributes
            .iter()
            .filter(|attribute| attribute.key == "rtpmap")
            .filter_map(|attribute| parse_rtpmap(attribute.value.as_deref()?))
        {
            if name.eq_ignore_ascii_case("opus") {
                found.opus = Some(payload_type);
            }
            found.red |= name.eq_ignore_ascii_case("red");
        }
        found
    }
}

/// `111 opus/48000/2` as `(111, "opus")`.
fn parse_rtpmap(value: &str) -> Option<(u8, &str)> {
    let (payload_type, encoding) = value.split_once(' ')?;
    let name = encoding.split('/').next()?;
    Some((payload_type.parse().ok()?, name))
}

/// What happens to each packet on its way to one participant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Repack {
    /// Sent as published.
    Unchanged,
    /// RED to a participant without it: only the current frame, as plain Opus.
    PrimaryOnly,
    /// RED kept, its blocks renumbered to the participant's Opus.
    RedFor { opus: u8 },
}

impl Repack {
    /// The packet as the participant expects it; `None` for a malformed RED
    /// payload, which is dropped.
    pub fn apply(self, mut packet: Packet) -> Option<Packet> {
        match self {
            Self::Unchanged => {}
            Self::PrimaryOnly => {
                // In range: `primary` measured this very payload.
                packet.payload = packet.payload.slice(red::primary(&packet.payload)?);
            }
            Self::RedFor { opus } => {
                packet.payload = red::with_block_payload_type(&packet.payload, opus)?.into();
            }
        }
        Some(packet)
    }
}

/// The codec an outlet for a track published as `source` is opened with, and
/// how its packets are repacked for that participant.
pub(super) fn outlet_codec(
    source: &RTCRtpCodecCapability,
    receiver: &ReceiverAudio,
) -> (RTCRtpCodecCapability, Repack) {
    if !source.mime_type.eq_ignore_ascii_case(MIME_TYPE_RED) {
        return (source.clone(), Repack::Unchanged);
    }
    if !receiver.red {
        return (opus(), Repack::PrimaryOnly);
    }
    let opus = receiver.opus.unwrap_or(OPUS_PAYLOAD_TYPE);
    (source.clone(), Repack::RedFor { opus })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write;
    use webrtc::api::media_engine::MIME_TYPE_VP8;

    /// A redundant block's first header byte: more blocks follow, then the payload type.
    const fn follows(payload_type: u8) -> u8 {
        0x80 | payload_type
    }

    fn sdp(audio_rtpmaps: &[&str]) -> String {
        let mut sdp = String::from(
            "v=0\r\no=- 1 2 IN IP4 127.0.0.1\r\ns=-\r\nt=0 0\r\n\
             m=video 9 UDP/TLS/RTP/SAVPF 96\r\na=rtpmap:96 VP8/90000\r\n\
             m=audio 9 UDP/TLS/RTP/SAVPF 63 109\r\n",
        );
        for rtpmap in audio_rtpmaps {
            let _ = write!(sdp, "a=rtpmap:{rtpmap}\r\n");
        }
        sdp.push_str("m=audio 9 UDP/TLS/RTP/SAVPF 111\r\na=rtpmap:111 opus/48000/2\r\n");
        sdp
    }

    fn capability(mime_type: &str) -> RTCRtpCodecCapability {
        RTCRtpCodecCapability { mime_type: mime_type.to_owned(), ..Default::default() }
    }

    #[test]
    fn the_first_audio_section_tells_the_receivers_codecs() {
        let audio = ReceiverAudio::from_sdp(&sdp(&["63 red/48000/2", "109 opus/48000/2"]));
        assert_eq!(audio, ReceiverAudio { opus: Some(109), red: true });

        let plain = ReceiverAudio::from_sdp(&sdp(&["109 opus/48000/2"]));
        assert_eq!(plain, ReceiverAudio { opus: Some(109), red: false });
    }

    #[test]
    fn an_unreadable_description_means_no_red() {
        assert_eq!(ReceiverAudio::from_sdp("not sdp"), ReceiverAudio::default());
    }

    #[test]
    fn tracks_other_than_red_are_sent_as_published() {
        let red_receiver = ReceiverAudio { opus: Some(111), red: true };
        for mime_type in [MIME_TYPE_OPUS, MIME_TYPE_VP8] {
            let (codec, repack) = outlet_codec(&capability(mime_type), &red_receiver);
            assert_eq!(codec.mime_type, mime_type);
            assert_eq!(repack, Repack::Unchanged);
        }
    }

    #[test]
    fn red_reaches_a_receiver_without_it_as_opus() {
        let (codec, repack) = outlet_codec(&capability(MIME_TYPE_RED), &ReceiverAudio::default());
        assert_eq!(codec.mime_type, MIME_TYPE_OPUS);
        assert_eq!(repack, Repack::PrimaryOnly);
    }

    #[test]
    fn red_stays_red_in_the_receivers_numbering() {
        let receiver = ReceiverAudio { opus: Some(109), red: true };
        let (codec, repack) = outlet_codec(&capability(MIME_TYPE_RED), &receiver);
        assert_eq!(codec.mime_type, MIME_TYPE_RED);
        assert_eq!(repack, Repack::RedFor { opus: 109 });

        let unnumbered = ReceiverAudio { opus: None, red: true };
        assert_eq!(
            outlet_codec(&capability(MIME_TYPE_RED), &unnumbered).1,
            Repack::RedFor { opus: OPUS_PAYLOAD_TYPE }
        );
    }

    #[test]
    fn repacking_changes_only_the_payload() {
        let packet = Packet {
            header: webrtc::rtp::header::Header { sequence_number: 7, ..Default::default() },
            payload: vec![follows(111), 0x00, 0x3c, 0x01, 111, 1, 9].into(),
        };
        let primary = Repack::PrimaryOnly.apply(packet.clone()).unwrap();
        assert_eq!(primary.payload.as_ref(), &[9]);
        assert_eq!(primary.header.sequence_number, 7);

        let renumbered = Repack::RedFor { opus: 109 }.apply(packet.clone()).unwrap();
        assert_eq!(renumbered.payload.as_ref(), &[follows(109), 0x00, 0x3c, 0x01, 109, 1, 9]);

        assert_eq!(Repack::Unchanged.apply(packet.clone()).unwrap(), packet);
    }

    #[test]
    fn a_malformed_red_packet_is_dropped() {
        let packet = Packet { payload: vec![follows(111)].into(), ..Default::default() };
        assert!(Repack::PrimaryOnly.apply(packet.clone()).is_none());
        assert!(Repack::RedFor { opus: 111 }.apply(packet).is_none());
    }

    #[test]
    fn the_engine_registers_red_next_to_the_defaults() {
        let mut engine = MediaEngine::default();
        register(&mut engine).unwrap();
    }
}
