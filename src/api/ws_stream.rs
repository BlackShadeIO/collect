//! WebSocket streaming endpoint with subscription filters.

use std::collections::HashSet;

use axum::{
    extract::{Query, State, WebSocketUpgrade},
    response::Response,
};
use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tracing::{info, warn};

use crate::api::server::AppState;

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

    loop {
        tokio::select! {
            // Forward broadcast messages to client (filtered)
            msg = broadcast_rx.recv() => {
                match msg {
                    Ok(record) => {
                        if !subscribed.contains(&record.source) {
                            continue;
                        }
                        if let Ok(json) = serde_json::to_string(&record) {
                            if sender.send(Message::Text(json.into())).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        warn!(skipped = n, "API WS client lagged");
                    }
                    Err(_) => break,
                }
            }

            // Handle client messages (subscribe/unsubscribe)
            client_msg = receiver.next() => {
                match client_msg {
                    Some(Ok(Message::Text(text))) => {
                        let text_str: &str = &text;
                        if let Ok(cmd) = serde_json::from_str::<FilterCommand>(text_str) {
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
struct FilterCommand {
    subscribe: Option<Vec<String>>,
    unsubscribe: Option<Vec<String>>,
}
