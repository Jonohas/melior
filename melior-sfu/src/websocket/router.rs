use std::future::Future;
use std::pin::Pin;
use axum::extract::ws::{Message, WebSocket};
use serde_json::Value;
use crate::handlers::echo::echo_strategy::EchoStrategy;
use crate::handlers::web_rtc::answer_sdp_offer_strategy::AnswerSDPOfferStrategy;
use crate::handlers::websocket_strategy::WebsocketStrategy;
use crate::websocket::message::{IncomingMessage, OutgoingMessage};

type HandlerFn = Box<
    dyn Fn(Value) -> Pin<Box<dyn Future<Output = Option<Value>> + Send>>
    + Send
    + Sync,
>;

pub struct WebsocketMessageRouter {
}

impl WebsocketMessageRouter {
    pub async fn handle(socket: &mut WebSocket, msg: &IncomingMessage) {
        match msg.channel.as_str() {
            "echo" => {
                let strategy = EchoStrategy::new();
                strategy.check_format(socket, msg.payload.clone(), msg.channel.clone()).await
            }
            "sdp" => {
                let strategy = AnswerSDPOfferStrategy::new();
                strategy.check_format(socket, msg.payload.clone(), msg.channel.clone()).await
            }
            _ => eprintln!("[Websocket] Unknown channel: {}", msg.channel),
        }
    }
}