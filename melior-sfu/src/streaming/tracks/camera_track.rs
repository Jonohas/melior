use std::sync::Arc;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::track::track_local::track_local_static_rtp::TrackLocalStaticRTP;
use crate::streaming::tracks::track::Track;

pub struct CameraTrack {
    peer_connection: Arc<RTCPeerConnection>,
    pub track: Arc<TrackLocalStaticRTP>,
}
impl Track for CameraTrack {
    fn new (peer_connection: Arc<RTCPeerConnection>) -> Self {
        let video_track = Arc::new(TrackLocalStaticRTP::new(
            RTCRtpCodecCapability {
                mime_type: "video/VP8".to_string(),
                ..Default::default()
            },
            "video".to_string(),
            "echo-stream".to_string(),
        ));

        Self { peer_connection, track: video_track }
    }

    async fn register_track(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let _ = self.peer_connection.add_track(self.track.clone()).await?;

        Ok(())
    }

    fn get_track(&self) -> Arc<TrackLocalStaticRTP> {
        self.track.clone()
    }
}