//! Binance BTC/USDT trade stream collector.

use tokio::sync::{broadcast, mpsc, watch};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::types::StorageRecord;
use crate::types::binance::BinanceTrade;
use crate::ws::connection::{WsConnection, WsEvent};

const BINANCE_TRADE_URL: &str = "wss://stream.binance.com:9443/ws/btcusdt@trade";

pub fn spawn_trade_collector(
    storage_tx: mpsc::Sender<StorageRecord>,
    trade_tx: mpsc::Sender<BinanceTrade>,
    broadcast_tx: broadcast::Sender<StorageRecord>,
    epoch_rx: watch::Receiver<u64>,
    token: CancellationToken,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        info!("Binance trade collector starting");
        let mut ws = WsConnection::connect_passive(BINANCE_TRADE_URL, token.clone());

        loop {
            tokio::select! {
                _ = token.cancelled() => {
                    info!("Binance trade collector shutting down");
                    return;
                }
                event = ws.next_event() => {
                    match event {
                        Some(WsEvent::Message(value)) => {
                            let ts = chrono::Utc::now().timestamp_millis();
                            let epoch = *epoch_rx.borrow();

                            // Try to deserialize as typed struct
                            if let Ok(trade) = serde_json::from_value::<BinanceTrade>(value.clone()) {
                                let _ = trade_tx.send(trade).await;
                            }

                            let record = StorageRecord {
                                ts,
                                source: "binance_trade".to_string(),
                                epoch: Some(epoch),
                                data: value,
                            };

                            let _ = storage_tx.send(record.clone()).await;
                            let _ = broadcast_tx.send(record);
                        }
                        Some(WsEvent::Connected) => {
                            info!("Binance trade WS connected");
                        }
                        Some(WsEvent::Disconnected) => {
                            warn!("Binance trade WS disconnected (will reconnect)");
                        }
                        Some(WsEvent::Error(e)) => {
                            warn!(error = %e, "Binance trade WS error");
                        }
                        None => {
                            info!("Binance trade WS channel closed");
                            return;
                        }
                    }
                }
            }
        }
    })
}
