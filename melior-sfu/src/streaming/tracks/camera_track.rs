use std::error::Error;
use std::sync::Arc;
use uuid::Uuid;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::track::track_local::track_local_static_rtp::TrackLocalStaticRTP;
use crate::streaming::tracks::track::Track;

pub struct CameraTrack<'a> {
    peer_connection: &'a RTCPeerConnection,
    pub stream: Arc<TrackLocalStaticRTP>,
    pub session_id: String,
}
impl<'a> Track<'a> for CameraTrack<'a> {
    fn new (peer_connection: &'a RTCPeerConnection, stream_id: String) -> Self {
        let video_track = Arc::new(TrackLocalStaticRTP::new(
            RTCRtpCodecCapability {
                mime_type: "video/VP8".to_string(),
                ..Default::default()
            },
            Uuid::new_v4().to_string(),
            stream_id.to_string(),
        ));

        Self { peer_connection, stream: video_track, session_id: stream_id }
    }

    async fn register_track(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        println!("Registering camera track with session_id: {}", self.session_id);
        self.peer_connection.add_track(self.stream.clone()).await?;
        Ok(())
    }

    fn get_track(&self) -> Arc<TrackLocalStaticRTP> {
        self.stream.clone()
    }

    fn set_session_id(&self) -> String {
        self.session_id.clone()
    }
}