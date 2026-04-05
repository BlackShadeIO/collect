//! Single market collection process lifecycle.

use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::polymarket::collector::spawn_clob_collector;
use crate::types::StorageRecord;
use crate::types::gamma::BtcMarket;

/// State of a market collection process.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub enum ProcessState {
    PreMarket,
    Live,
    Draining,
    Done,
}

/// A running collection process for a single market.
pub struct MarketProcess {
    pub epoch: u64,
    pub market: BtcMarket,
    pub state: ProcessState,
    token: CancellationToken,
    handle: JoinHandle<()>,
}

impl MarketProcess {
    /// Start collecting for a market. Spawns a CLOB collector task.
    pub fn start(
        market: BtcMarket,
        storage_tx: mpsc::Sender<StorageRecord>,
        broadcast_tx: broadcast::Sender<StorageRecord>,
        parent_token: &CancellationToken,
    ) -> Self {
        let epoch = market.epoch;
        let token = parent_token.child_token();
        let asset_ids = vec![
            market.yes_token_id.clone(),
            market.no_token_id.clone(),
        ];

        info!(
            epoch,
            slug = %market.slug,
            question = %market.question,
            "Starting market collection process"
        );

        let handle = spawn_clob_collector(
            epoch,
            asset_ids,
            storage_tx,
            broadcast_tx,
            token.clone(),
        );

        Self {
            epoch,
            market,
            state: ProcessState::PreMarket,
            token,
            handle,
        }
    }

    /// Cancel the process and wait for it to drain.
    pub async fn stop(self) {
        info!(epoch = self.epoch, "Stopping market process");
        self.token.cancel();
        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            self.handle,
        ).await;
    }

    pub fn cancel(&self) {
        self.token.cancel();
    }

    pub fn is_finished(&self) -> bool {
        self.handle.is_finished()
    }
}
