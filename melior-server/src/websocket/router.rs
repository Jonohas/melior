use crate::handlers::echo::echo_strategy::EchoStrategy;
use crate::handlers::web_rtc::answer_sdp_offer_strategy::AnswerSDPOfferStrategy;
use crate::handlers::web_rtc::ice_candidate_strategy::IceCandidateStrategy;
use crate::handlers::websocket_strategy::WebsocketStrategy;
use crate::structs::app_state::AppState;
use crate::websocket::message::IncomingMessage;
use axum::extract::ws::WebSocket;
use crate::handlers::web_rtc::handle_sdp_answer_strategy::HandleSDPAnswerStrategy;

pub struct WebsocketMessageRouter {}

impl WebsocketMessageRouter {
    pub async fn handle(socket: &mut WebSocket, msg: &IncomingMessage, state: &AppState) {
        match msg.channel.as_str() {
            "echo" => {
                let strategy = EchoStrategy::new();
                strategy
                    .check_format(
                        socket,
                        msg.payload.clone(),
                        msg.channel.clone(),
                        &msg.session_id,
                        &msg.channel_id,
                        state,
                    )
                    .await
            }
            "sdp" => {
                let strategy = AnswerSDPOfferStrategy::new();
                strategy
                    .check_format(
                        socket,
                        msg.payload.clone(),
                        msg.channel.clone(),
                        &msg.session_id,
                        &msg.channel_id,
                        state,
                    )
                    .await
            }
            "ice" => {
                let strategy = IceCandidateStrategy::new();
                strategy
                    .check_format(
                        socket,
                        msg.payload.clone(),
                        msg.channel.clone(),
                        &msg.session_id,
                        &msg.channel_id,
                        state,
                    )
                    .await
            }
            
            "sdp_answer" => {
                let strategy = HandleSDPAnswerStrategy::new();
                strategy
                    .check_format(
                        socket,
                        msg.payload.clone(),
                        msg.channel.clone(),
                        &msg.session_id,
                        &msg.channel_id,
                        state,
                    )
                    .await
            }
            _ => eprintln!("[Websocket] Unknown channel: {}", msg.channel),
        }
    }
}
