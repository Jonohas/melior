mod structs;
mod shared;
mod strategies;

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use fred::error::{Error};
use fred::prelude::ClientLike;
use tokio::signal;
use crate::shared::redis_manager::RedisManager;
use crate::shared::webrtc_manager::WebRtcManager;
use crate::structs::app_state::AppState;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let manager = Arc::new(RedisManager::new("127.0.0.1", 6379));
    let web_rtc_manager = Arc::new(WebRtcManager::new());

    let app_state = Arc::new(AppState {
        redis_manager: manager.clone(),
        web_rtc_manager: web_rtc_manager.clone(),
    });

    manager.init().await?;
    manager.register_handlers(app_state).await;

    println!("Application running...");

    // Wait for Ctrl+C
    signal::ctrl_c().await.expect("Failed to listen for event");

    println!("Shutting down...");
    // Graceful shutdown
    manager.disconnect().await?;

    Ok(())
}

// add more sfu logic here