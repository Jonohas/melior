mod structs;
mod redis_manager;
mod shared;
mod strategies;

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use fred::error::{Error};
use fred::prelude::ClientLike;
use tokio::signal;
use crate::redis_manager::RedisManager;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let manager = Arc::new(RedisManager::new("127.0.0.1", 6379));
    manager.init(manager.clone()).await?;
    manager.connect().await?;

    println!("Application running...");

    // Wait for Ctrl+C
    signal::ctrl_c().await.expect("Failed to listen for event");

    println!("Shutting down...");
    // Graceful shutdown
    manager.disconnect().await?;

    Ok(())
}

// add more sfu logic here