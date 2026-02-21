use std::sync::Arc;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::track::track_local::track_local_static_rtp::TrackLocalStaticRTP;

pub enum TrackType {
    Audio,
    Camera,
    ScreenShare,
}

pub trait Track {
    fn new(peer_connection: Arc<RTCPeerConnection>) -> Self
    where
        Self: Sized;
    async fn register_track(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn get_track(&self) -> Arc<TrackLocalStaticRTP>;
}