use crate::streaming::tracks::track::Track;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast::Sender;
use webrtc::rtp::packet::Packet;

pub struct Channel {
    // session_id: String,
    pub tracks: HashMap<String, Sender<Packet>>,
}
impl<'a> Channel {
    pub fn new(_: String) -> Self {
        Self {
            tracks: HashMap::new(),
        }
    }

    pub fn add_track<T: Track>(&mut self, track: Arc<T>) {
        self.tracks
            .insert(track.set_session_id(), track.get_sender());
    }
}
