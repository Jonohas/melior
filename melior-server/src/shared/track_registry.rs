use crate::tui::EventEmitter;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use webrtc::track::track_local::track_local_static_rtp::TrackLocalStaticRTP;
use webrtc::track::track_local::TrackLocal;

#[derive(Clone)]
pub struct TrackRegistry {
    pub tracks: Arc<Mutex<HashMap<String, Vec<Arc<TrackLocalStaticRTP>>>>>,
    pub event_emitter: Arc<Mutex<EventEmitter>>,
}

impl TrackRegistry {
    pub fn new(emitter: Arc<Mutex<EventEmitter>>) -> Self {
        Self {
            tracks: Arc::new(Mutex::new(HashMap::new())),
            event_emitter: emitter,
        }
    }

    pub async fn add_track(&self, session_id: &str, track: Arc<TrackLocalStaticRTP>) {
        let mut tracks = self.tracks.lock().await;
        let codec = track.codec();
        let codec_str = codec.mime_type.to_string();
        let track_id = track.id().to_string();

        tracks
            .entry(session_id.to_string())
            .or_default()
            .push(track);

        let emitter = self.event_emitter.lock().await;
        emitter.emit(crate::tui::TuiEvent::TrackAdded {
            session_id: session_id.to_string(),
            codec: codec_str,
            track_id,
        });
    }

    pub async fn get_other_tracks(&self, session_id: &str) -> Vec<Arc<TrackLocalStaticRTP>> {
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

    pub async fn get_all_tracks(&self) -> Vec<(String, Arc<TrackLocalStaticRTP>)> {
        let tracks = self.tracks.lock().await;
        let mut result = Vec::new();
        for (session_id, session_tracks) in tracks.iter() {
            for track in session_tracks {
                result.push((session_id.clone(), track.clone()));
            }
        }
        result
    }

    pub async fn remove_session(&self, session_id: &str) {
        let mut tracks = self.tracks.lock().await;
        if let Some(session_tracks) = tracks.remove(session_id) {
            let emitter = self.event_emitter.lock().await;
            for track in session_tracks {
                let track_id = track.id().to_string();
                emitter.emit(crate::tui::TuiEvent::TrackRemoved {
                    session_id: session_id.to_string(),
                    track_id,
                });
            }
        }
    }
}
