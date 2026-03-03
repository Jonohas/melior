use crate::shared::webrtc_manager::WebRtcManager;
use crate::structs::client_registry::ClientRegistry;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct AppState {
    pub web_rtc_manager: Arc<Mutex<WebRtcManager>>,
    pub client_registry: Arc<ClientRegistry>,
}
