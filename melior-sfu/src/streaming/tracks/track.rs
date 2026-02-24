use std::sync::Arc;
use tokio::sync::broadcast;
use webrtc::rtp::packet::Packet;
use webrtc::track::track_remote::TrackRemote;

pub trait Track {
    fn new(stream: Arc<TrackRemote>, session_id: String) -> Self
    where
        Self: Sized;

    fn set_session_id(&self) -> String;
    fn get_sender(&self) -> broadcast::Sender<Packet>;
}