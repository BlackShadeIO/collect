//! Generic WebSocket connection manager with heartbeat and auto-reconnect.
//! Adapted from poly_order/src/ws/client.rs with CancellationToken support.

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

/// Commands sent to the WebSocket background task.
#[derive(Debug)]
pub enum WsCommand {
    SendJson(serde_json::Value),
    Close,
}

/// Events emitted by the WebSocket background task.
#[derive(Debug)]
pub enum WsEvent {
    Connected,
    Message(serde_json::Value),
    Disconnected,
    Error(String),
}

/// Heartbeat mode for different WebSocket protocols.
#[derive(Debug, Clone, Copy)]
pub enum HeartbeatMode {
    /// Send text "PING", expect text "PONG" (Polymarket protocol).
    TextPing,
    /// Only respond to server WebSocket-level pings (Binance protocol).
    /// No client-initiated pings needed.
    ServerPingOnly,
}

/// Handle to a managed WebSocket connection.
pub struct WsConnection {
    pub cmd_tx: mpsc::Sender<WsCommand>,
    pub msg_rx: mpsc::Receiver<WsEvent>,
}

const PING_INTERVAL: Duration = Duration::from_secs(10);
const PONG_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_BACKOFF: Duration = Duration::from_secs(30);
const INITIAL_BACKOFF: Duration = Duration::from_secs(1);

impl WsConnection {
    /// Connect with text PING heartbeat (Polymarket).
    pub fn connect(url: &str, token: CancellationToken) -> Self {
        Self::connect_with_heartbeat(url, HeartbeatMode::TextPing, token)
    }

    /// Connect with server-only ping handling (Binance).
    pub fn connect_passive(url: &str, token: CancellationToken) -> Self {
        Self::connect_with_heartbeat(url, HeartbeatMode::ServerPingOnly, token)
    }

    fn connect_with_heartbeat(url: &str, heartbeat: HeartbeatMode, token: CancellationToken) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel::<WsCommand>(64);
        let (msg_tx, msg_rx) = mpsc::channel::<WsEvent>(256);

        let url_owned = url.to_string();
        tokio::spawn(ws_task(url_owned, cmd_rx, msg_tx, token, heartbeat));

        Self { cmd_tx, msg_rx }
    }

    pub async fn send_json(&self, value: serde_json::Value) -> Result<(), String> {
        self.cmd_tx
            .send(WsCommand::SendJson(value))
            .await
            .map_err(|e| format!("failed to send command: {e}"))
    }

    pub async fn next_event(&mut self) -> Option<WsEvent> {
        self.msg_rx.recv().await
    }

    pub async fn close(&self) {
        let _ = self.cmd_tx.send(WsCommand::Close).await;
    }
}

async fn ws_task(
    url: String,
    mut cmd_rx: mpsc::Receiver<WsCommand>,
    msg_tx: mpsc::Sender<WsEvent>,
    token: CancellationToken,
    heartbeat: HeartbeatMode,
) {
    let mut backoff = INITIAL_BACKOFF;

    loop {
        if token.is_cancelled() {
            debug!("WS task cancelled before connect");
            return;
        }

        info!(url = %url, "connecting to WebSocket");
        let ws_stream = match connect_async(&url).await {
            Ok((stream, _)) => {
                info!("WebSocket connected");
                backoff = INITIAL_BACKOFF;
                let _ = msg_tx.send(WsEvent::Connected).await;
                stream
            }
            Err(e) => {
                error!(error = %e, "WebSocket connection failed");
                let _ = msg_tx.send(WsEvent::Error(format!("connection failed: {e}"))).await;

                tokio::select! {
                    _ = tokio::time::sleep(backoff) => {}
                    _ = token.cancelled() => return,
                }
                backoff = advance_backoff(backoff);
                continue;
            }
        };

        let (mut ws_sink, mut ws_stream_rx) = ws_stream.split();

        let should_reconnect = run_session(
            &mut ws_sink,
            &mut ws_stream_rx,
            &mut cmd_rx,
            &msg_tx,
            &token,
            heartbeat,
        )
        .await;

        let _ = msg_tx.send(WsEvent::Disconnected).await;

        if !should_reconnect {
            debug!("WS task shutting down (clean close)");
            return;
        }

        warn!(backoff_secs = backoff.as_secs(), "reconnecting after backoff");

        tokio::select! {
            _ = tokio::time::sleep(backoff) => {}
            _ = token.cancelled() => return,
        }
        backoff = advance_backoff(backoff);
    }
}

async fn run_session(
    ws_sink: &mut futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        Message,
    >,
    ws_stream_rx: &mut futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
    cmd_rx: &mut mpsc::Receiver<WsCommand>,
    msg_tx: &mpsc::Sender<WsEvent>,
    token: &CancellationToken,
    heartbeat: HeartbeatMode,
) -> bool {
    let mut ping_interval = tokio::time::interval(PING_INTERVAL);
    ping_interval.tick().await; // consume first tick

    let mut awaiting_pong = false;
    let mut pong_deadline = tokio::time::Instant::now() + PONG_TIMEOUT;

    loop {
        tokio::select! {
            _ = token.cancelled() => {
                debug!("CancellationToken triggered — closing WS");
                let _ = ws_sink.close().await;
                return false; // clean shutdown
            }

            _ = ping_interval.tick() => {
                match heartbeat {
                    HeartbeatMode::TextPing => {
                        if awaiting_pong {
                            if tokio::time::Instant::now() >= pong_deadline {
                                warn!("PONG timeout — reconnecting");
                                let _ = ws_sink.close().await;
                                return true;
                            }
                        } else {
                            debug!("sending text PING");
                            if let Err(e) = ws_sink.send(Message::Text("PING".into())).await {
                                error!(error = %e, "failed to send PING");
                                return true;
                            }
                            awaiting_pong = true;
                            pong_deadline = tokio::time::Instant::now() + PONG_TIMEOUT;
                        }
                    }
                    HeartbeatMode::ServerPingOnly => {
                        // No client-initiated pings. Just a liveness check:
                        // if we haven't received anything in a long time, the
                        // stream.next() arm will return None and we'll reconnect.
                    }
                }
            }

            msg = ws_stream_rx.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        let text_str: &str = &text;
                        if text_str == "PONG" {
                            debug!("received text PONG");
                            awaiting_pong = false;
                        } else {
                            match serde_json::from_str::<serde_json::Value>(text_str) {
                                Ok(value) => {
                                    let _ = msg_tx.send(WsEvent::Message(value)).await;
                                }
                                Err(_) => {
                                    let _ = msg_tx.send(WsEvent::Message(
                                        serde_json::Value::String(text_str.to_string()),
                                    )).await;
                                }
                            }
                        }
                    }
                    Some(Ok(Message::Ping(data))) => {
                        // Respond to WebSocket-level pings (both protocols)
                        let _ = ws_sink.send(Message::Pong(data)).await;
                    }
                    Some(Ok(Message::Close(_))) => {
                        info!("server sent Close frame");
                        return true;
                    }
                    Some(Err(e)) => {
                        error!(error = %e, "WebSocket read error");
                        return true;
                    }
                    None => {
                        info!("WebSocket stream ended");
                        return true;
                    }
                    _ => {}
                }
            }

            cmd = cmd_rx.recv() => {
                match cmd {
                    Some(WsCommand::SendJson(value)) => {
                        let text = serde_json::to_string(&value).unwrap_or_default();
                        debug!(msg = %text, "sending message");
                        if let Err(e) = ws_sink.send(Message::Text(text.into())).await {
                            error!(error = %e, "failed to send message");
                            return true;
                        }
                    }
                    Some(WsCommand::Close) => {
                        info!("close command received");
                        let _ = ws_sink.close().await;
                        return false;
                    }
                    None => {
                        info!("command channel closed");
                        let _ = ws_sink.close().await;
                        return false;
                    }
                }
            }
        }
    }
}

fn advance_backoff(current: Duration) -> Duration {
    let next = current * 2;
    if next > MAX_BACKOFF { MAX_BACKOFF } else { next }
}
