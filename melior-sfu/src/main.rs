mod structs;
mod redis_manager;

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use fred::error::{Error};
use fred::prelude::ClientLike;
use tokio::signal;
use tokio::sync::RwLock;
use crate::redis_manager::RedisManager;
use crate::structs::sfu_manager::SFUManager;

type SharedState = Arc<RwLock<SFUManager>>;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let manager = RedisManager::new("127.0.0.1", 6379);
    manager.connect().await?;

    println!("Application running...");

    // Wait for Ctrl+C
    signal::ctrl_c().await.expect("Failed to listen for event");

    println!("Shutting down...");
    // Graceful shutdown
    manager.client().quit().await?;

    Ok(())
}

// add more sfu logic here