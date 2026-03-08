use crate::shared::track_registry::TrackRegistry;
use crate::shared::webrtc_manager::WebRtcManager;
use crate::structs::client_registry::ClientRegistry;
use crate::tui::EventEmitter;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct AppState {
    pub web_rtc_manager: Arc<Mutex<WebRtcManager>>,
    pub client_registry: Arc<ClientRegistry>,
    pub track_registry: TrackRegistry,
    pub event_emitter: Arc<Mutex<EventEmitter>>,
}
