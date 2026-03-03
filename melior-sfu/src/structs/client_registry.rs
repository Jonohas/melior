use axum::extract::ws::Message;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct ClientRegistry {
    session_senders: Arc<Mutex<HashMap<String, tokio::sync::mpsc::Sender<Message>>>>,
}

impl ClientRegistry {
    pub fn new() -> Self {
        Self {
            session_senders: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn register(&self, session_id: String, sender: tokio::sync::mpsc::Sender<Message>) {
        self.session_senders.lock().await.insert(session_id, sender);
    }

    pub async fn send_to_session(
        &self,
        session_id: &str,
        message: Message,
    ) -> Result<(), tokio::sync::mpsc::error::SendError<Message>> {
        let senders = self.session_senders.lock().await;
        if let Some(sender) = senders.get(session_id) {
            sender.send(message).await?;
        }
        Ok(())
    }

    pub async fn unregister(&self, session_id: &str) {
        self.session_senders.lock().await.remove(session_id);
    }
}
