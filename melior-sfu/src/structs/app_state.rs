use std::sync::Arc;
use crate::shared::redis_manager::RedisManager;
use crate::shared::webrtc_manager::WebRtcManager;

pub struct AppState {
    pub redis_manager: Arc<RedisManager>,
    pub web_rtc_manager: Arc<WebRtcManager>,
}