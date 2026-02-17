use fred::prelude::*;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::SFUManager; // Assumes your state struct is in main.rs or a state.rs

pub struct RedisManager {
    client: Client,
}


// receives the events from the dotnet application and creates the appropriate hashmap entries
impl RedisManager {
    // Similar to a Constructor
    pub fn new(host: &str, port: u16) -> Self {
        let config = Config {
            server: ServerConfig::Centralized {
                server: (host, port).into(),
            },
            ..Default::default()
        };

        let client = Client::new(config, None, None, None);

        Self { client }
    }

    pub async fn connect(&self) -> Result<(), Error> {
        // fred clients are handles to a background task.
        // connect() returns a future that resolves when the connection is established.
        self.client.connect();
        self.client.wait_for_connect().await
    }

    /// Accessor for the internal client to perform operations.
    pub fn client(&self) -> &Client {
        &self.client
    }
}