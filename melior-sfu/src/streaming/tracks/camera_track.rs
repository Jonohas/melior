use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::broadcast::Sender;
use webrtc::rtp::packet::Packet;
use webrtc::track::track_remote::TrackRemote;
use crate::streaming::tracks::track::{Track};

pub struct CameraTrack {
    pub stream: Arc<TrackRemote>,
    pub session_id: String,
    pub sender: broadcast::Sender<Packet>,
}
impl Track for CameraTrack {
    fn new (stream: Arc<TrackRemote>, stream_id: String) -> Self {
        let (tx, _) = broadcast::channel(64);
        Self { stream, sender: tx, session_id: stream_id }
    }

    fn set_session_id(&self) -> String {
        self.session_id.clone()
    }

    fn get_sender(&self) -> Sender<Packet> {
        self.sender.clone()
    }
}