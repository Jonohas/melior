//! Cleaning up images that were uploaded but never sent: an image is uploaded
//! before its message, and a message that is never sent leaves it unattached.

use super::ports::ChatStoreError;
use super::service::ChatService;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio::time::MissedTickBehavior;
use tracing::{info, warn};

/// How many images one sweep removes at most; the next sweep takes the rest.
const SWEEP_BATCH: usize = 200;

impl ChatService {
    /// Removes up to 200 images (`SWEEP_BATCH`) that no message carries and that
    /// were uploaded more than `older_than` ago, and returns how many. The
    /// records go first, in one step, so no message can attach them any more;
    /// then the bytes. Bytes that cannot be removed are logged and left behind:
    /// nothing names them, so they are only wasted space.
    pub async fn sweep_unattached_images(
        &self,
        older_than: Duration,
    ) -> Result<usize, ChatStoreError> {
        let ids = self.messages.delete_unattached_images(older_than, SWEEP_BATCH).await?;
        // One request per image, deliberately: S3's batch delete needs a signed
        // XML body with Content-MD5, and a sweep is bounded by SWEEP_BATCH.
        for id in &ids {
            if let Err(error) = self.images.delete(id).await {
                warn!(%error, image = %id, "unattached image bytes not removed");
            }
        }
        Ok(ids.len())
    }
}

/// Sweeps unattached images every `every`, removing those older than
/// `older_than`. Every instance may run one: each image is claimed by one
/// sweep. A failing store is logged and tried again next time. Must be called
/// within a tokio runtime.
pub fn start_image_sweeper(
    chat: Arc<ChatService>,
    older_than: Duration,
    every: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticks = tokio::time::interval(every);
        ticks.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            ticks.tick().await;
            match chat.sweep_unattached_images(older_than).await {
                Ok(0) => {}
                Ok(removed) => info!(removed, "unattached images removed"),
                Err(error) => warn!(%error, "unattached image sweep failed"),
            }
        }
    })
}
