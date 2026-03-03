use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Deserialize)]
pub struct IncomingMessage {
    pub channel: String,
    pub payload: Value,
}

#[derive(Serialize)]
pub struct OutgoingMessage {
    pub channel: String,
    pub payload: Value,
}