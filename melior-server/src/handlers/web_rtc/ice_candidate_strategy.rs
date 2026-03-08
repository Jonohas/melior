use crate::handlers::websocket_strategy::WebsocketStrategy;
use crate::structs::app_state::AppState;
use axum::extract::ws::WebSocket;
use serde::Deserialize;
use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;

#[derive(Deserialize)]
pub struct IceCandidateMessage {
    pub candidate: String,
    pub sdp_mid: Option<String>,
    pub sdp_mline_index: Option<u16>,
}

pub struct IceCandidateStrategy;

impl WebsocketStrategy for IceCandidateStrategy {
    type Message = IceCandidateMessage;

    fn new() -> Self {
        Self
    }

    async fn handle_message(
        &self,
        socket: &mut WebSocket,
        message: Self::Message,
        session_id: &str,
        _channel_id: &str,
        state: &AppState,
    ) {
        let manager = state.web_rtc_manager.lock().await;
        let peers = manager.peers.lock().await;

        if let Some(peer) = peers.get(session_id) {
            let init = RTCIceCandidateInit {
                candidate: message.candidate,
                sdp_mid: message.sdp_mid,
                sdp_mline_index: message.sdp_mline_index,
                username_fragment: None,
            };

            if let Err(e) = peer.add_ice_candidate(init).await {
                let emitter = state.event_emitter.lock().await;
                emitter.emit_log("ERROR", &format!("Failed to add ICE candidate: {}", e));
            }
        }
    }
}
