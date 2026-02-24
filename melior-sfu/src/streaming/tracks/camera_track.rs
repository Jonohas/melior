use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::broadcast::Sender;
use webrtc::rtp::packet::Packet;
use webrtc::track::track_remote::TrackRemote;
use crate::streaming::tracks::track::{Track};

pub struct CameraTrack {
    pub stream: Arc<TrackRemote>,
    pub session_id: String,
    pub sender: Sender<Packet>,
    _receiver: broadcast::Receiver<Packet>, // Keep channel open
}
impl Track for CameraTrack {
    fn new (stream: Arc<TrackRemote>, stream_id: String) -> Self {
        let (tx, rx) = broadcast::channel(64);
        Self { 
            stream, 
            sender: tx, 
            session_id: stream_id,
            _receiver: rx,
        }
    }

    fn set_session_id(&self) -> String {
        self.session_id.clone()
    }

    fn get_sender(&self) -> Sender<Packet> {
        self.sender.clone()
    }
}