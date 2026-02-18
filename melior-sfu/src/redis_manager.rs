use fred::prelude::*;
use fred::clients::SubscriberClient;
use crate::structs::redis_strategy::{ChannelHandler, HandleMessage};
use crate::structs::sfu_strategy::SFUStrategy;

pub struct RedisManager {
    client: Client,
    subscriber_client: SubscriberClient,
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

        let client = Client::new(config.clone(), None, None, None);
        let subscriber_client = SubscriberClient::new(config, None, None, None);

        Self { client, subscriber_client }
    }

    pub async fn init(&self) -> Result<(), Error> {
        self.subscriber_client.init().await?;

        let mut message_rx = self.subscriber_client.message_rx();
        tokio::spawn(async move {
            while let Ok(message) = message_rx.recv().await {
                ChannelHandler::handle(message);
            }
        });

        let _ = self.subscriber_client.subscribe("ChannelConnectionChannel").await?;

        Ok(())
    }

    pub async fn connect(&self) -> Result<(), Error> {
        // fred clients are handles to a background task.
        // connect() returns a future that resolves when the connection is established.
        self.client.connect();
        self.client.wait_for_connect().await
    }

    pub async fn disconnect(&self) -> Result<(), Error> {
        self.client.quit().await?;
        self.subscriber_client.unsubscribe_all().await?;
        self.subscriber_client.quit().await?;

        Ok(())
    }

    /// Accessor for the internal client to perform operations.
    pub fn client(&self) -> &Client {
        &self.client
    }

    pub fn subscriber_client(&self) -> &SubscriberClient {
        &self.subscriber_client
    }
}