use std::collections::HashMap;
use std::sync::Arc;
use webrtc::api::API;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::track::track_local::track_local_static_rtp::TrackLocalStaticRTP;
use crate::streaming::tracks::camera_track::CameraTrack;
use crate::streaming::tracks::track::{Track, TrackType};

pub struct Peer {
    peer_connection: Arc<RTCPeerConnection>,
    incoming_tracks: HashMap<String, Arc<TrackLocalStaticRTP>>,
}
impl Peer {
    pub async fn new(api: Arc<API>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let pc = Arc::new(api.new_peer_connection(RTCConfiguration::default()).await?);
        let mut ic: HashMap<String, Arc<TrackLocalStaticRTP>> = HashMap::new();
        let track: CameraTrack = Self::create_track(pc.clone());
        track.register_track().await?;
        ic.insert("camera".to_string(), track.get_track());

        let s = Self { peer_connection: pc, incoming_tracks: ic };
        Ok(s)
    }

    pub fn get_track(&self) -> Option<&Arc<TrackLocalStaticRTP>> {
        self.incoming_tracks.get("camera")
    }

    pub fn get_peer_connection(&self) -> Arc<RTCPeerConnection> {
        self.peer_connection.clone()
    }

    fn create_track<T: Track>(peer_connection: Arc<RTCPeerConnection>) -> T {
        T::new(peer_connection)
    }
}