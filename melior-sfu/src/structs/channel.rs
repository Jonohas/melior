use std::collections::HashMap;
use std::sync::Arc;
use webrtc::track::track_local::track_local_static_rtp::TrackLocalStaticRTP;
use crate::streaming::tracks::track::Track;

pub struct Channel {
    // session_id: String,
    pub tracks: HashMap<String, Arc<TrackLocalStaticRTP>>,
}
impl<'a> Channel {
    pub fn new(_: String) -> Self {
        Self { tracks: HashMap::new() }
    }

    pub fn add_track<T: Track<'a>>(&mut self, track: Arc<T>) {
        self.tracks.insert(track.set_session_id(), track.get_track());
    }
}