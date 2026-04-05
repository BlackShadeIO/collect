//! WebSocket streaming endpoint with subscription filters and backfill support.

use std::collections::HashSet;
use std::time::Duration;

use axum::{
    extract::{Query, State, WebSocketUpgrade},
    response::Response,
};
use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tracing::{info, warn};

use crate::api::server::AppState;
use crate::types::StorageRecord;
use crate::types::gamma::current_epoch;

/// Interval at which buffered records are flushed to the client.
const FLUSH_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Deserialize)]
pub struct WsParams {
    pub token: String,
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(params): Query<WsParams>,
    State(state): State<AppState>,
) -> Response {
    // Auth via query param
    if params.token != state.config.api_key {
        return Response::builder()
            .status(401)
            .body("Unauthorized".into())
            .unwrap();
    }

    ws.on_upgrade(move |socket| handle_ws(socket, state))
}

async fn handle_ws(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    let mut broadcast_rx = state.broadcast_tx.subscribe();

    // Default: subscribe to all sources
    let mut subscribed: HashSet<String> = HashSet::from([
        "binance_trade".to_string(),
        "binance_depth".to_string(),
        "polymarket_clob".to_string(),
        "calculation".to_string(),
    ]);

    info!("API WebSocket client connected");

    let mut flush_tick = tokio::time::interval(FLUSH_INTERVAL);
    flush_tick.tick().await; // consume first immediate tick
    let mut buffer: Vec<StorageRecord> = Vec::new();

    loop {
        tokio::select! {
            // Collect broadcast messages into the buffer (no send yet)
            msg = broadcast_rx.recv() => {
                match msg {
                    Ok(record) => {
                        if subscribed.contains(&record.source) {
                            buffer.push(record);
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        warn!(skipped = n, "API WS client lagged");
                    }
                    Err(_) => break,
                }
            }

            // Flush buffered records to the client every 100ms
            _ = flush_tick.tick() => {
                if buffer.is_empty() {
                    continue;
                }
                for record in buffer.drain(..) {
                    if let Ok(json) = serde_json::to_string(&record) {
                        if sender.send(Message::Text(json.into())).await.is_err() {
                            break;
                        }
                    }
                }
            }

            // Handle client messages (subscribe/unsubscribe/backfill)
            client_msg = receiver.next() => {
                match client_msg {
                    Some(Ok(Message::Text(text))) => {
                        let text_str: &str = &text;
                        if let Ok(cmd) = serde_json::from_str::<ClientCommand>(text_str) {
                            if let Some(sources) = cmd.subscribe {
                                for s in sources {
                                    subscribed.insert(s);
                                }
                            }
                            if let Some(sources) = cmd.unsubscribe {
                                for s in sources {
                                    subscribed.remove(&s);
                                }
                            }
                            if let Some(backfill) = cmd.backfill {
                                if let Err(e) = send_backfill(&state, &backfill, &mut sender).await {
                                    warn!(error = %e, "Failed to send backfill");
                                }
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }

    info!("API WebSocket client disconnected");
}

#[derive(Deserialize)]
struct ClientCommand {
    subscribe: Option<Vec<String>>,
    unsubscribe: Option<Vec<String>>,
    backfill: Option<BackfillRequest>,
}

#[derive(Deserialize)]
struct BackfillRequest {
    /// How many seconds of history to send. Defaults to 120.
    last_seconds: Option<u64>,
    /// Which sources to include. Defaults to all.
    sources: Option<Vec<String>>,
    /// Specific epoch to backfill from. Defaults to current epoch.
    epoch: Option<u64>,
}

/// Read recent records from JSONL files and send them over the WebSocket.
async fn send_backfill(
    state: &AppState,
    req: &BackfillRequest,
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
) -> anyhow::Result<()> {
    let epoch = req.epoch.unwrap_or_else(current_epoch);
    let last_seconds = req.last_seconds.unwrap_or(120);
    let cutoff_ms = chrono::Utc::now().timestamp_millis() - (last_seconds as i64 * 1000);

    let all_sources: Vec<String> = vec![
        "binance_trade".into(),
        "binance_depth".into(),
        "polymarket_clob".into(),
        "calculation".into(),
    ];
    let sources = req.sources.as_ref().unwrap_or(&all_sources);

    let data_dir = &state.config.data_dir;
    let epoch_dir = data_dir.join(epoch.to_string());

    if !epoch_dir.exists() {
        // Send empty backfill_end marker
        let _ = sender
            .send(Message::Text(
                serde_json::json!({"backfill_end": true, "count": 0}).to_string().into(),
            ))
            .await;
        return Ok(());
    }

    let mut records: Vec<(i64, String)> = Vec::new(); // (ts, json_line) for sorting

    for source in sources {
        let path = epoch_dir.join(format!("{source}.jsonl"));
        if let Ok(content) = tokio::fs::read(&path).await {
            for line in content.split(|&b| b == b'\n') {
                if line.is_empty() {
                    continue;
                }
                // Quick timestamp check before full parse
                if let Ok(v) = serde_json::from_slice::<serde_json::Value>(line) {
                    if let Some(ts) = v.get("ts").and_then(|t| t.as_i64()) {
                        if ts >= cutoff_ms {
                            if let Ok(s) = std::str::from_utf8(line) {
                                records.push((ts, s.to_string()));
                            }
                        }
                    }
                }
            }
        }
    }

    // Sort by timestamp so client receives records in order
    records.sort_by_key(|(ts, _)| *ts);

    let count = records.len();
    for (_, json_line) in records {
        if sender.send(Message::Text(json_line.into())).await.is_err() {
            return Ok(()); // Client disconnected
        }
    }

    // Send backfill_end marker so client knows when to switch to live mode
    let _ = sender
        .send(Message::Text(
            serde_json::json!({"backfill_end": true, "count": count}).to_string().into(),
        ))
        .await;

    info!(epoch = epoch, count = count, last_seconds = last_seconds, "Backfill sent");
    Ok(())
}
