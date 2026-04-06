//! Technical indicator computation engine.
//! Computes time-based moving averages (7s, 25s, 99s), RSI-14, strike price,
//! and depth imbalance.

use std::collections::VecDeque;
use std::time::Instant;

use ta::indicators::RelativeStrengthIndex;
use ta::Next;
use tokio::sync::{broadcast, mpsc, watch};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::types::{DepthState, IndicatorSnapshot, StorageRecord};
use crate::types::binance::BinanceTrade;

/// A timestamped price sample for time-window moving averages.
struct PriceSample {
    ts_ms: i64,
    price: f64,
}

/// Rolling time-window simple moving average.
struct TimeWindowMA {
    window_ms: i64,
    samples: VecDeque<PriceSample>,
}

impl TimeWindowMA {
    fn new(window_secs: u64) -> Self {
        Self {
            window_ms: window_secs as i64 * 1000,
            samples: VecDeque::new(),
        }
    }

    fn push(&mut self, ts_ms: i64, price: f64) {
        self.samples.push_back(PriceSample { ts_ms, price });
        self.evict(ts_ms);
    }

    fn evict(&mut self, now_ms: i64) {
        let cutoff = now_ms - self.window_ms;
        while self.samples.front().is_some_and(|s| s.ts_ms < cutoff) {
            self.samples.pop_front();
        }
    }

    fn value(&self) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.samples.iter().map(|s| s.price).sum();
        sum / self.samples.len() as f64
    }
}

pub fn spawn_calculation_engine(
    mut trade_rx: mpsc::Receiver<BinanceTrade>,
    depth_rx: watch::Receiver<DepthState>,
    storage_tx: mpsc::Sender<StorageRecord>,
    indicator_tx: watch::Sender<IndicatorSnapshot>,
    broadcast_tx: broadcast::Sender<StorageRecord>,
    epoch_rx: watch::Receiver<u64>,
    token: CancellationToken,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        info!("Calculation engine starting");

        let mut rsi_14 = RelativeStrengthIndex::new(14).unwrap();
        let mut ma_7s = TimeWindowMA::new(7);
        let mut ma_25s = TimeWindowMA::new(25);
        let mut ma_99s = TimeWindowMA::new(99);

        let mut last_emit = Instant::now();
        let mut last_price: f64 = 0.0;

        // Strike tracking: first BTC price per epoch
        let mut current_epoch: u64 = 0;
        let mut strike: f64 = 0.0;

        loop {
            tokio::select! {
                _ = token.cancelled() => {
                    info!("Calculation engine shutting down");
                    return;
                }
                trade = trade_rx.recv() => {
                    match trade {
                        Some(trade) => {
                            let price = match trade.price.parse::<f64>() {
                                Ok(p) => p,
                                Err(_) => {
                                    warn!(raw = %trade.price, "Failed to parse trade price");
                                    continue;
                                }
                            };

                            let epoch = *epoch_rx.borrow();
                            let ts = chrono::Utc::now().timestamp_millis();

                            // Detect epoch change → set strike to first price
                            if epoch != current_epoch && epoch > 0 {
                                strike = price;
                                current_epoch = epoch;
                                info!(epoch = epoch, strike = strike, "New market epoch — strike price set");
                            }

                            last_price = price;
                            let _ = rsi_14.next(price);

                            // Feed time-based MAs
                            ma_7s.push(ts, price);
                            ma_25s.push(ts, price);
                            ma_99s.push(ts, price);

                            // Emit snapshot at most once per second
                            if last_emit.elapsed().as_secs() >= 1 {
                                let depth = depth_rx.borrow().clone();

                                let total = depth.total_bid_qty + depth.total_ask_qty;
                                let depth_imbalance = if total > 0.0 {
                                    (depth.total_bid_qty - depth.total_ask_qty) / total
                                } else {
                                    0.0
                                };

                                let snapshot = IndicatorSnapshot {
                                    ts,
                                    epoch,
                                    btc_price: last_price,
                                    strike,
                                    ma_7s: ma_7s.value(),
                                    ma_25s: ma_25s.value(),
                                    ma_99s: ma_99s.value(),
                                    rsi_14: rsi_14.next(last_price),
                                    depth_imbalance,
                                    mid_price: depth.mid_price,
                                    best_bid: depth.best_bid,
                                    best_ask: depth.best_ask,
                                };

                                indicator_tx.send_modify(|s| *s = snapshot.clone());

                                let record = StorageRecord {
                                    ts,
                                    source: "calculation".to_string(),
                                    epoch: Some(epoch),
                                    data: serde_json::to_value(&snapshot)
                                        .unwrap_or(serde_json::Value::Null),
                                };

                                let _ = storage_tx.send(record.clone()).await;
                                let _ = broadcast_tx.send(record);

                                last_emit = Instant::now();
                            }
                        }
                        None => {
                            info!("Trade channel closed, calculation engine stopping");
                            return;
                        }
                    }
                }
            }
        }
    })
}
