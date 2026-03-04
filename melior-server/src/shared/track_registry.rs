use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use webrtc::track::track_local::track_local_static_rtp::TrackLocalStaticRTP;

#[derive(Default, Clone)]
pub struct TrackRegistry {
    /// session_id -> list of forwarder tracks published by that session
    pub tracks: Arc<Mutex<HashMap<String, Vec<Arc<TrackLocalStaticRTP>>>>>,
}

impl TrackRegistry {
    pub async fn add_track(&self, session_id: &str, track: Arc<TrackLocalStaticRTP>) {
        let mut tracks = self.tracks.lock().await;
        tracks
            .entry(session_id.to_string())
            .or_default()
            .push(track);
    }

    /// Returns all tracks NOT belonging to the given session
    pub async fn get_other_tracks(
        &self,
        session_id: &str,
    ) -> Vec<Arc<TrackLocalStaticRTP>> {
        let tracks = self.tracks.lock().await;
        tracks
            .iter()
            .filter(|(id, _)| id.as_str() != session_id)
            .flat_map(|(_, t)| t.clone())
            .collect()
    }

    pub async fn get_all_session_ids(&self) -> Vec<String> {
        let tracks = self.tracks.lock().await;
        tracks.keys().cloned().collect()
    }
}