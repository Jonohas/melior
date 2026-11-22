use super::backend;
use super::codecs;
use super::forwarder::ForwarderRegistry;
use super::session::WebRtcSession;
use async_trait::async_trait;
use melior_application::{MediaEngine, MediaError, MediaEvents, MediaSession};
use melior_domain::SessionId;
use std::sync::{Arc, Weak};
use webrtc::api::interceptor_registry::{configure_nack, configure_twcc_receiver_only};
use webrtc::api::media_engine::MediaEngine as RtcMediaEngine;
use webrtc::api::{API, APIBuilder};
use webrtc::interceptor::registry::Registry;
use webrtc::interceptor::report::receiver::ReceiverReport;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::rtp_transceiver::rtp_codec::{RTCRtpHeaderExtensionCapability, RTPCodecType};
use webrtc::sdp::extmap::{SDES_MID_URI, SDES_RTP_STREAM_ID_URI};

/// [`MediaEngine`] backed by webrtc-rs: one peer connection per participant.
pub struct WebRtcMediaEngine {
    api: API,
    forwarders: ForwarderRegistry,
}

/// The header extensions a simulcast camera needs: the MID names the section and
/// the RID the layer, since its layers carry no SSRC in the offer.
fn register_simulcast_extensions(media_engine: &mut RtcMediaEngine) -> Result<(), MediaError> {
    for uri in [SDES_MID_URI, SDES_RTP_STREAM_ID_URI] {
        let extension = RTCRtpHeaderExtensionCapability { uri: uri.to_owned() };
        media_engine
            .register_header_extension(extension, RTPCodecType::Video, None)
            .map_err(backend)?;
    }
    Ok(())
}

/// webrtc-rs's default interceptors minus its sender reports: NACK, receiver
/// reports and TWCC feedback, without which there is no loss recovery. Its sender
/// reports would pair each RTP time with the moment the SFU forwarded the packet,
/// which hides how much later than its sound the publisher sent a frame. Outlets
/// pass on the publisher's own reports instead (`OutletTrack::sender_report`).
fn interceptors(media_engine: &mut RtcMediaEngine) -> Result<Registry, MediaError> {
    let registry = configure_nack(Registry::new(), media_engine);
    let mut registry = configure_twcc_receiver_only(registry, media_engine).map_err(backend)?;
    registry.add(Box::new(ReceiverReport::builder()));
    Ok(registry)
}

impl WebRtcMediaEngine {
    pub fn new() -> Result<Self, MediaError> {
        let mut media_engine = RtcMediaEngine::default();
        codecs::register(&mut media_engine).map_err(backend)?;
        register_simulcast_extensions(&mut media_engine)?;
        let interceptors = interceptors(&mut media_engine)?;
        let api = APIBuilder::new()
            .with_media_engine(media_engine)
            .with_interceptor_registry(interceptors)
            .build();

        Ok(Self { api, forwarders: ForwarderRegistry::default() })
    }
}

#[async_trait]
impl MediaEngine for WebRtcMediaEngine {
    async fn create_session(
        &self,
        session_id: SessionId,
        events: Weak<dyn MediaEvents>,
    ) -> Result<Arc<dyn MediaSession>, MediaError> {
        // No ICE servers: the SFU must be directly reachable and offers its host
        // candidates. TURN is for clients; they relay towards those candidates.
        let connection = Arc::new(
            self.api.new_peer_connection(RTCConfiguration::default()).await.map_err(backend)?,
        );
        Ok(Arc::new(WebRtcSession::new(session_id, connection, self.forwarders.clone(), events)))
    }
}
