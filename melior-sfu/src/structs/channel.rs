use std::collections::HashMap;
use std::sync::Arc;
use webrtc::track::track_local::track_local_static_rtp::TrackLocalStaticRTP;

pub struct Channel {
    // Key: UserID, Value: The track they are broadcasting
    pub broadcasters: HashMap<String, Arc<TrackLocalStaticRTP>>,
}