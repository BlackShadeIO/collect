//! Polymarket CLOB WebSocket collector task.
//! Subscribes to orderbook data for specific token IDs.

use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::types::StorageRecord;
use crate::ws::connection::{WsConnection, WsEvent};

const POLYMARKET_WS_URL: &str = "wss://ws-subscriptions-clob.polymarket.com/ws/market";

pub fn spawn_clob_collector(
    epoch: u64,
    asset_ids: Vec<String>,
    storage_tx: mpsc::Sender<StorageRecord>,
    broadcast_tx: broadcast::Sender<StorageRecord>,
    token: CancellationToken,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        info!(epoch, asset_count = asset_ids.len(), "Polymarket CLOB collector starting");
        let mut ws = WsConnection::connect(POLYMARKET_WS_URL, token.clone());

        // Wait for connection, then subscribe
        let mut subscribed = false;

        loop {
            tokio::select! {
                _ = token.cancelled() => {
                    info!(epoch, "Polymarket CLOB collector shutting down");
                    // Try to unsubscribe
                    let unsub = serde_json::json!({
                        "operation": "unsubscribe",
                        "assets_ids": asset_ids,
                    });
                    let _ = ws.send_json(unsub).await;
                    ws.close().await;
                    return;
                }
                event = ws.next_event() => {
                    match event {
                        Some(WsEvent::Connected) => {
                            info!(epoch, "Polymarket CLOB WS connected, subscribing...");
                            let sub_msg = serde_json::json!({
                                "assets_ids": asset_ids,
                                "type": "market",
                                "custom_feature_enabled": true,
                            });
                            if let Err(e) = ws.send_json(sub_msg).await {
                                warn!(error = %e, "Failed to send subscription");
                            } else {
                                subscribed = true;
                            }
                        }
                        Some(WsEvent::Message(value)) => {
                            if !subscribed {
                                continue;
                            }

                            let ts = chrono::Utc::now().timestamp_millis();

                            // The first message after subscribe is an array of book snapshots.
                            // Subsequent messages are individual events.
                            // Handle both: if it's an array, emit each element separately.
                            let values = if let Some(arr) = value.as_array() {
                                arr.clone()
                            } else {
                                vec![value]
                            };

                            for v in values {
                                let record = StorageRecord {
                                    ts,
                                    source: "polymarket_clob".to_string(),
                                    epoch: Some(epoch),
                                    data: v,
                                };
                                let _ = storage_tx.send(record.clone()).await;
                                let _ = broadcast_tx.send(record);
                            }
                        }
                        Some(WsEvent::Disconnected) => {
                            warn!(epoch, "Polymarket CLOB WS disconnected");
                            subscribed = false;
                            // WsConnection auto-reconnects, we'll re-subscribe on Connected
                        }
                        Some(WsEvent::Error(e)) => {
                            warn!(epoch, error = %e, "Polymarket CLOB WS error");
                        }
                        None => {
                            info!(epoch, "Polymarket CLOB WS channel closed");
                            return;
                        }
                    }
                }
            }
        }
    })
}
