use std::sync::Arc;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::track::track_local::track_local_static_rtp::TrackLocalStaticRTP;

pub trait Track<'a> {
    fn new(peer_connection: &'a RTCPeerConnection, session_id: String) -> Self
    where
        Self: Sized;
    async fn register_track(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn get_track(&self) -> Arc<TrackLocalStaticRTP>;
    fn set_session_id(&self) -> String;
}