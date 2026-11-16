use super::backend;
use super::forwarder::ForwarderRegistry;
use super::session::WebRtcSession;
use async_trait::async_trait;
use melior_application::{MediaEngine, MediaError, MediaEvents, MediaSession};
use melior_domain::SessionId;
use std::sync::{Arc, Weak};
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine as RtcMediaEngine;
use webrtc::api::{API, APIBuilder};
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;

/// [`MediaEngine`] backed by webrtc-rs: one peer connection per participant.
pub struct WebRtcMediaEngine {
    api: API,
    forwarders: ForwarderRegistry,
}

impl WebRtcMediaEngine {
    pub fn new() -> Result<Self, MediaError> {
        let mut media_engine = RtcMediaEngine::default();
        media_engine.register_default_codecs().map_err(backend)?;
        // NACK, RTCP reports and TWCC feedback: without these there is no loss recovery.
        let interceptors =
            register_default_interceptors(Registry::new(), &mut media_engine).map_err(backend)?;
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
