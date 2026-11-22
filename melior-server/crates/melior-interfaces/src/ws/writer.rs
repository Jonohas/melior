//! The writing half of a WebSocket: a queue drained by a task, so that nothing
//! that produces a message ever waits on a slow client. The queue holds at most
//! [`QUEUE_BUDGET_BYTES`]; a client that lets it fill is not reading, and its
//! connection ends instead of the server's memory growing without limit.

use axum::extract::ws::{Message, WebSocket};
use futures_util::SinkExt;
use futures_util::stream::SplitSink;
use serde::Serialize;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;
use tokio::sync::{Notify, mpsc};
use tokio::task::JoinHandle;
use tracing::warn;

/// How long a closing connection may take to flush what is queued.
const FLUSH: Duration = Duration::from_secs(5);

/// The most serialized bytes one socket may have queued and not yet sent. Far above
/// what a reading client ever has outstanding (the largest message is 128 KiB).
pub(super) const QUEUE_BUDGET_BYTES: usize = 4 * 1024 * 1024;

/// Counts the bytes queued for one socket. Once a message does not fit, the budget
/// stays spent: a connection that is about to end must not skip a message and then
/// deliver the ones after it.
#[derive(Debug)]
struct Budget {
    limit: usize,
    queued: AtomicUsize,
    spent: AtomicBool,
}

impl Budget {
    fn new(limit: usize) -> Self {
        Self { limit, queued: AtomicUsize::new(0), spent: AtomicBool::new(false) }
    }

    /// Takes room for `bytes`, or spends the budget for good if they do not fit.
    fn reserve(&self, bytes: usize) -> bool {
        if self.spent.load(Ordering::Acquire) {
            return false;
        }
        let fits = self
            .queued
            .try_update(Ordering::AcqRel, Ordering::Acquire, |queued| {
                queued.checked_add(bytes).filter(|total| *total <= self.limit)
            })
            .is_ok();
        if !fits {
            self.spent.store(true, Ordering::Release);
        }
        fits
    }

    /// Gives back the room of a message that has been sent.
    fn release(&self, bytes: usize) {
        self.queued.fetch_sub(bytes, Ordering::AcqRel);
    }
}

#[derive(Debug)]
struct Shared {
    budget: Budget,
    overflow: Notify,
}

/// The sending end of one socket's queue. Pushing never waits.
#[derive(Debug, Clone)]
pub(super) struct Outbound {
    tx: mpsc::UnboundedSender<String>,
    shared: Arc<Shared>,
}

impl Outbound {
    /// Queues `message` as JSON. If the queue is over its budget, the message is
    /// dropped and [`Outbound::overflowed`] resolves.
    pub(super) fn push(&self, message: impl Serialize) {
        let text = match serde_json::to_string(&message) {
            Ok(text) => text,
            Err(error) => {
                warn!(%error, "unserializable message");
                return;
            }
        };
        if !self.shared.budget.reserve(text.len()) {
            self.shared.overflow.notify_one();
            return;
        }
        // Fails only once the socket is gone, when there is nobody left to tell.
        let _ = self.tx.send(text);
    }

    /// Resolves once a message did not fit: the client is not reading, and its
    /// connection has to end.
    pub(super) async fn overflowed(&self) {
        self.shared.overflow.notified().await;
    }
}

/// Starts the task that sends every queued message, then closes the socket once
/// every sender is gone.
pub(super) fn spawn_writer(mut sink: SplitSink<WebSocket, Message>) -> (Outbound, JoinHandle<()>) {
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    let shared =
        Arc::new(Shared { budget: Budget::new(QUEUE_BUDGET_BYTES), overflow: Notify::new() });
    let drained = shared.clone();
    let writer = tokio::spawn(async move {
        while let Some(text) = rx.recv().await {
            let bytes = text.len();
            let sent = sink.send(Message::text(text)).await;
            drained.budget.release(bytes);
            if sent.is_err() {
                break;
            }
        }
        let _ = sink.close().await;
    });
    (Outbound { tx, shared }, writer)
}

/// Waits for the writer to flush and close, but not forever.
pub(super) async fn finish(mut writer: JoinHandle<()>) {
    if tokio::time::timeout(FLUSH, &mut writer).await.is_err() {
        writer.abort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_fit_up_to_the_limit() {
        let budget = Budget::new(10);
        assert!(budget.reserve(4));
        assert!(budget.reserve(6));
    }

    #[test]
    fn a_message_over_the_limit_spends_the_budget_for_good() {
        let budget = Budget::new(10);
        assert!(budget.reserve(8));
        assert!(!budget.reserve(3));
        budget.release(8);
        assert!(!budget.reserve(1), "nothing may follow a dropped message");
    }

    #[test]
    fn sent_messages_give_their_room_back() {
        let budget = Budget::new(10);
        assert!(budget.reserve(10));
        budget.release(10);
        assert!(budget.reserve(10));
    }

    #[tokio::test]
    async fn an_overflow_is_announced_even_before_anyone_waits() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let outbound = Outbound {
            tx,
            shared: Arc::new(Shared { budget: Budget::new(4), overflow: Notify::new() }),
        };
        outbound.push("far more than four bytes");
        let announced = tokio::time::timeout(Duration::from_secs(1), outbound.overflowed()).await;
        assert!(announced.is_ok());
    }
}
