//! The writing half of a WebSocket: a queue drained by a task, so that nothing
//! that produces a message ever waits on a slow client.

use axum::extract::ws::{Message, WebSocket};
use futures_util::SinkExt;
use futures_util::stream::SplitSink;
use serde::Serialize;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::warn;

/// How long a closing connection may take to flush what is queued.
const FLUSH: Duration = Duration::from_secs(5);

/// Starts the task that sends every queued message as JSON, then closes the socket
/// once every sender is gone.
pub(super) fn spawn_writer<M: Serialize + Send + 'static>(
    mut sink: SplitSink<WebSocket, Message>,
) -> (mpsc::UnboundedSender<M>, JoinHandle<()>) {
    let (tx, mut rx) = mpsc::unbounded_channel::<M>();
    let writer = tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            let text = match serde_json::to_string(&message) {
                Ok(text) => text,
                Err(error) => {
                    warn!(%error, "unserializable message");
                    continue;
                }
            };
            if sink.send(Message::text(text)).await.is_err() {
                break;
            }
        }
        let _ = sink.close().await;
    });
    (tx, writer)
}

/// Waits for the writer to flush and close, but not forever.
pub(super) async fn finish(mut writer: JoinHandle<()>) {
    if tokio::time::timeout(FLUSH, &mut writer).await.is_err() {
        writer.abort();
    }
}
