use std::sync::Arc;
use fred::prelude::*;
use fred::clients::SubscriberClient;
use serde::Serialize;
use serde_json;
use crate::shared::channel_constants::{CHANNEL_CONNECTION_CHANNEL, WEBRTC_CHANNEL};
use crate::shared::channel_handler::ChannelHandler;
use crate::structs::app_state::AppState;

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

    pub fn hello(&self) {
        println!("Hello from RedisManager!");
    }

    pub async fn init(&self) -> Result<(), Error> {
        self.subscriber_client.init().await?;

        self.connect().await?;
        self.subscribe_to_channels().await;

        Ok(())
    }

    pub async fn connect(&self) -> Result<(), Error> {
        // fred peer are handles to a background task.
        // connect() returns a future that resolves when the connection is established.
        self.client.connect();
        self.client.wait_for_connect().await?;

        // Connect the subscriber client
        self.subscriber_client.connect();
        self.subscriber_client.wait_for_connect().await
    }

    pub async fn disconnect(&self) -> Result<(), Error> {
        self.client.quit().await?;
        self.subscriber_client.unsubscribe_all().await?;
        self.subscriber_client.quit().await?;

        Ok(())
    }

    pub async fn publish<T>(&self, channel: &str, message: T) -> Result<(), Error>
    where
        T: Serialize,
    {
        let payload = serde_json::to_string(&message).map_err(|e| {
            // Note: ErrorKind is from fred::prelude::*
            Error::new(ErrorKind::Parse, e.to_string())
        })?;

        // Explicitly specify <(), _> to tell the compiler the return type is ()
        // and the input type should be inferred (the payload string)
        self.client.publish::<(), _, _>(channel, payload).await?;

        Ok(())
    }
    /// Accessor for the internal client to perform operations.
    pub fn client(&self) -> &Client {
        &self.client
    }

    pub fn subscriber_client(&self) -> &SubscriberClient {
        &self.subscriber_client
    }

    pub async fn subscribe_to_channels(&self) {
        let _ = self.subscriber_client.subscribe(CHANNEL_CONNECTION_CHANNEL).await;
        let _ = self.subscriber_client.subscribe(WEBRTC_CHANNEL).await;
    }

    pub async fn register_handlers(&self, app_state: Arc<AppState>) {
        let mut message_rx = self.subscriber_client.message_rx();

        tokio::spawn(async move {
            while let Ok(message) = message_rx.recv().await {
                ChannelHandler::handle(message, app_state.clone()).await;
            }
        });
    }
}