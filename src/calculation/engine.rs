//! Technical indicator computation engine.
//! Computes EMA-9, EMA-21, RSI-14 on trade prices and depth imbalance.

use std::time::Instant;

use ta::indicators::{ExponentialMovingAverage, RelativeStrengthIndex};
use ta::Next;
use tokio::sync::{broadcast, mpsc, watch};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::types::{DepthState, IndicatorSnapshot, StorageRecord};
use crate::types::binance::BinanceTrade;

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

        let mut ema_9 = ExponentialMovingAverage::new(9).unwrap();
        let mut ema_21 = ExponentialMovingAverage::new(21).unwrap();
        let mut rsi_14 = RelativeStrengthIndex::new(14).unwrap();

        let mut last_emit = Instant::now();
        let mut last_price;
        let mut last_ema_9;
        let mut last_ema_21;
        let mut last_rsi_14;

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

                            last_price = price;
                            last_ema_9 = ema_9.next(price);
                            last_ema_21 = ema_21.next(price);
                            last_rsi_14 = rsi_14.next(price);

                            // Emit snapshot at most once per second
                            if last_emit.elapsed().as_secs() >= 1 {
                                let depth = depth_rx.borrow().clone();
                                let epoch = *epoch_rx.borrow();
                                let ts = chrono::Utc::now().timestamp_millis();

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
                                    ema_9: last_ema_9,
                                    ema_21: last_ema_21,
                                    rsi_14: last_rsi_14,
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
