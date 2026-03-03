use std::sync::Arc;
use tokio::sync::Mutex;
use crate::shared::webrtc_manager::WebRtcManager;

pub struct AppState {
    pub web_rtc_manager: Arc<Mutex<WebRtcManager>>,
}