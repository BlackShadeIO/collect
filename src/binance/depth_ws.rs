//! Binance BTC/USDT depth stream collector.
//! Uses depth20@100ms which sends full top-20 snapshots.

use tokio::sync::{broadcast, mpsc, watch};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::types::{DepthState, StorageRecord};
use crate::types::binance::BinanceDepthSnapshot;
use crate::ws::connection::{WsConnection, WsEvent};

const BINANCE_DEPTH_URL: &str = "wss://stream.binance.com:9443/ws/btcusdt@depth20@100ms";

pub fn spawn_depth_collector(
    storage_tx: mpsc::Sender<StorageRecord>,
    depth_state_tx: watch::Sender<DepthState>,
    broadcast_tx: broadcast::Sender<StorageRecord>,
    epoch_rx: watch::Receiver<u64>,
    token: CancellationToken,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        info!("Binance depth collector starting");
        let mut ws = WsConnection::connect_passive(BINANCE_DEPTH_URL, token.clone());

        loop {
            tokio::select! {
                _ = token.cancelled() => {
                    info!("Binance depth collector shutting down");
                    return;
                }
                event = ws.next_event() => {
                    match event {
                        Some(WsEvent::Message(value)) => {
                            let ts = chrono::Utc::now().timestamp_millis();
                            let epoch = *epoch_rx.borrow();

                            // Update depth state from snapshot
                            if let Ok(snap) = serde_json::from_value::<BinanceDepthSnapshot>(value.clone()) {
                                let state = compute_depth_state(&snap);
                                depth_state_tx.send_modify(|s| *s = state);
                            }

                            let record = StorageRecord {
                                ts,
                                source: "binance_depth".to_string(),
                                epoch: Some(epoch),
                                data: value,
                            };

                            let _ = storage_tx.send(record.clone()).await;
                            let _ = broadcast_tx.send(record);
                        }
                        Some(WsEvent::Connected) => {
                            info!("Binance depth WS connected");
                        }
                        Some(WsEvent::Disconnected) => {
                            warn!("Binance depth WS disconnected (will reconnect)");
                        }
                        Some(WsEvent::Error(e)) => {
                            warn!(error = %e, "Binance depth WS error");
                        }
                        None => {
                            info!("Binance depth WS channel closed");
                            return;
                        }
                    }
                }
            }
        }
    })
}

fn compute_depth_state(snap: &BinanceDepthSnapshot) -> DepthState {
    let best_bid = snap.bids.first()
        .and_then(|b| b[0].parse::<f64>().ok())
        .unwrap_or(0.0);
    let best_ask = snap.asks.first()
        .and_then(|a| a[0].parse::<f64>().ok())
        .unwrap_or(0.0);

    let total_bid_qty: f64 = snap.bids.iter()
        .filter_map(|b| b[1].parse::<f64>().ok())
        .sum();
    let total_ask_qty: f64 = snap.asks.iter()
        .filter_map(|a| a[1].parse::<f64>().ok())
        .sum();

    let mid_price = if best_bid > 0.0 && best_ask > 0.0 {
        (best_bid + best_ask) / 2.0
    } else {
        0.0
    };

    DepthState {
        best_bid,
        best_ask,
        total_bid_qty,
        total_ask_qty,
        mid_price,
    }
}
