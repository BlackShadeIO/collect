//! 3-slot process lifecycle manager for market rotation.

use tokio::sync::{broadcast, mpsc, watch};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::gamma::client::GammaClient;
use crate::types::StorageRecord;
use crate::types::gamma::{current_epoch, next_epoch, seconds_remaining};

use super::process::{MarketProcess, ProcessState};

/// Orchestrator state published via watch channel for the API.
#[derive(Debug, Clone, serde::Serialize, Default)]
pub struct OrchestratorState {
    pub slots: Vec<SlotInfo>,
    pub current_epoch: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SlotInfo {
    pub slot: usize,
    pub epoch: u64,
    pub slug: String,
    pub state: ProcessState,
    pub seconds_remaining: i64,
}

pub struct Scheduler {
    slots: [Option<MarketProcess>; 3],
    gamma: GammaClient,
    storage_tx: mpsc::Sender<StorageRecord>,
    broadcast_tx: broadcast::Sender<StorageRecord>,
    epoch_tx: watch::Sender<u64>,
    orch_state_tx: watch::Sender<OrchestratorState>,
    root_token: CancellationToken,
}

impl Scheduler {
    pub fn new(
        gamma: GammaClient,
        storage_tx: mpsc::Sender<StorageRecord>,
        broadcast_tx: broadcast::Sender<StorageRecord>,
        epoch_tx: watch::Sender<u64>,
        orch_state_tx: watch::Sender<OrchestratorState>,
        root_token: CancellationToken,
    ) -> Self {
        Self {
            slots: [None, None, None],
            gamma,
            storage_tx,
            broadcast_tx,
            epoch_tx,
            orch_state_tx,
            root_token,
        }
    }

    /// Run the orchestrator loop. Returns when cancelled.
    pub async fn run(&mut self) {
        info!("Orchestrator starting");

        // Initial market discovery
        let cur = current_epoch();
        let rem = seconds_remaining(cur);
        info!(epoch = cur, seconds_remaining = rem, "Current 5-minute window");

        // Start first process
        let first_epoch = if rem > 60 { cur } else { next_epoch(cur) };
        self.try_start_process(first_epoch).await;

        // Main loop — tick every second
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));

        loop {
            tokio::select! {
                _ = self.root_token.cancelled() => {
                    info!("Orchestrator shutting down");
                    self.stop_all().await;
                    return;
                }
                _ = interval.tick() => {
                    self.tick().await;
                }
            }
        }
    }

    async fn tick(&mut self) {
        let cur = current_epoch();

        // Update epoch watch for Binance file tagging
        self.epoch_tx.send_modify(|e| *e = cur);

        // Process state transitions
        for i in 0..3 {
            if let Some(proc) = &mut self.slots[i] {
                let rem = seconds_remaining(proc.epoch);

                // Transition PreMarket -> Live when market starts
                if proc.state == ProcessState::PreMarket && rem <= 300 {
                    proc.state = ProcessState::Live;
                    info!(slot = i, epoch = proc.epoch, "Market is now LIVE");
                }

                // Transition Live -> Draining when market ends
                if proc.state == ProcessState::Live && rem <= 0 {
                    proc.state = ProcessState::Draining;
                    info!(slot = i, epoch = proc.epoch, "Market ended, draining");
                    proc.cancel();
                }

                // Transition Draining -> Done after 5 seconds or task finished
                if proc.state == ProcessState::Draining && (rem <= -5 || proc.is_finished()) {
                    proc.state = ProcessState::Done;
                    info!(slot = i, epoch = proc.epoch, "Process done");
                }
            }
        }

        // Clean up Done slots
        for i in 0..3 {
            if self.slots[i].as_ref().is_some_and(|p| p.state == ProcessState::Done) {
                if let Some(proc) = self.slots[i].take() {
                    proc.stop().await;
                }
            }
        }

        // Start next market process if needed
        // Find the highest epoch among active processes
        let max_epoch = self.slots.iter()
            .filter_map(|s| s.as_ref().map(|p| p.epoch))
            .max()
            .unwrap_or(cur);

        let next_ep = next_epoch(max_epoch);
        let next_rem = seconds_remaining(max_epoch);

        // When the latest tracked market has < 60s remaining, start the next one
        if next_rem < 60 && !self.has_epoch(next_ep) && self.free_slot().is_some() {
            self.try_start_process(next_ep).await;
        }

        // If no processes are running, start the next upcoming market
        if self.active_count() == 0 {
            let upcoming = next_epoch(cur);
            self.try_start_process(upcoming).await;
        }

        // Publish orchestrator state
        self.publish_state();
    }

    async fn try_start_process(&mut self, epoch: u64) {
        if self.has_epoch(epoch) {
            return;
        }
        let slot = match self.free_slot() {
            Some(s) => s,
            None => {
                warn!("No free slot for epoch {epoch}");
                return;
            }
        };

        match self.gamma.find_market_with_retry(epoch, 3).await {
            Ok(market) => {
                let proc = MarketProcess::start(
                    market,
                    self.storage_tx.clone(),
                    self.broadcast_tx.clone(),
                    &self.root_token,
                );
                info!(slot, epoch, "Process started in slot");
                self.slots[slot] = Some(proc);
            }
            Err(e) => {
                warn!(epoch, error = %e, "Failed to find market");
            }
        }
    }

    fn has_epoch(&self, epoch: u64) -> bool {
        self.slots.iter().any(|s| s.as_ref().is_some_and(|p| p.epoch == epoch))
    }

    fn free_slot(&self) -> Option<usize> {
        self.slots.iter().position(|s| s.is_none())
    }

    fn active_count(&self) -> usize {
        self.slots.iter().filter(|s| s.is_some()).count()
    }

    async fn stop_all(&mut self) {
        for i in 0..3 {
            if let Some(proc) = self.slots[i].take() {
                proc.stop().await;
            }
        }
    }

    fn publish_state(&self) {
        let slots: Vec<SlotInfo> = self.slots.iter().enumerate()
            .filter_map(|(i, s)| {
                s.as_ref().map(|p| SlotInfo {
                    slot: i,
                    epoch: p.epoch,
                    slug: p.market.slug.clone(),
                    state: p.state,
                    seconds_remaining: seconds_remaining(p.epoch),
                })
            })
            .collect();

        let state = OrchestratorState {
            slots,
            current_epoch: current_epoch(),
        };

        self.orch_state_tx.send_modify(|s| *s = state);
    }
}
