mod api;
mod binance;
mod calculation;
mod config;
mod gamma;
mod orchestrator;
mod polymarket;
mod storage;
mod types;
mod ws;

use std::time::Instant;

use anyhow::Result;
use tokio::sync::{broadcast, mpsc, watch};
use tokio_util::sync::CancellationToken;
use tracing_subscriber::EnvFilter;

use crate::api::server::{AppState, build_app};
use crate::binance::depth_ws::spawn_depth_collector;
use crate::binance::trade_ws::spawn_trade_collector;
use crate::calculation::engine::spawn_calculation_engine;
use crate::config::AppConfig;
use crate::gamma::client::GammaClient;
use crate::orchestrator::scheduler::{OrchestratorState, Scheduler};
use crate::storage::writer::StorageWriter;
use crate::types::{DepthState, IndicatorSnapshot, StorageRecord};
use crate::types::binance::BinanceTrade;
use crate::types::gamma::current_epoch;

#[tokio::main]
async fn main() -> Result<()> {
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install default CryptoProvider");

    let config = AppConfig::from_env()?;

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new(&config.log_level)),
        )
        .init();

    tokio::fs::create_dir_all(&config.data_dir).await?;

    tracing::info!(
        data_dir = %config.data_dir.display(),
        listen = %config.listen_addr(),
        "poly-collect starting"
    );

    let started_at = Instant::now();
    let root_token = CancellationToken::new();

    // --- Channels ---
    let (trade_tx, trade_rx) = mpsc::channel::<BinanceTrade>(2048);
    let (broadcast_tx, _) = broadcast::channel::<StorageRecord>(256);
    let (epoch_tx, epoch_rx) = watch::channel::<u64>(current_epoch());
    let (depth_state_tx, depth_state_rx) = watch::channel(DepthState::default());
    let (indicator_tx, indicator_rx) = watch::channel(IndicatorSnapshot {
        ts: 0, epoch: 0, btc_price: 0.0, strike: 0.0,
        ma_7s: 0.0, ma_25s: 0.0, ma_99s: 0.0,
        rsi_14: 0.0, depth_imbalance: 0.0, mid_price: 0.0, best_bid: 0.0, best_ask: 0.0,
    });
    let (orch_state_tx, orch_state_rx) = watch::channel(OrchestratorState::default());

    // --- Spawn StorageWriter ---
    let writer = StorageWriter::spawn(config.data_dir.clone(), root_token.child_token());
    let storage_tx = writer.sender();

    // --- Spawn Binance collectors (persistent) ---
    let _trade_handle = spawn_trade_collector(
        storage_tx.clone(),
        trade_tx,
        broadcast_tx.clone(),
        epoch_rx.clone(),
        root_token.child_token(),
    );

    let _depth_handle = spawn_depth_collector(
        storage_tx.clone(),
        depth_state_tx,
        broadcast_tx.clone(),
        epoch_rx.clone(),
        root_token.child_token(),
    );

    // --- Spawn Calculation Engine ---
    let _calc_handle = spawn_calculation_engine(
        trade_rx,
        depth_state_rx.clone(),
        storage_tx.clone(),
        indicator_tx,
        broadcast_tx.clone(),
        epoch_rx,
        root_token.child_token(),
    );

    // --- Spawn API Server ---
    let app_state = AppState {
        config: config.clone(),
        started_at,
        broadcast_tx: broadcast_tx.clone(),
        orch_state_rx: orch_state_rx.clone(),
        indicator_rx: indicator_rx.clone(),
        depth_state_rx: depth_state_rx.clone(),
    };

    let app = build_app(app_state);
    let listen_addr = config.listen_addr();
    let listener = tokio::net::TcpListener::bind(&listen_addr).await?;
    tracing::info!(addr = %listen_addr, "API server listening");

    let api_token = root_token.child_token();
    let _api_handle = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async move { api_token.cancelled().await })
            .await
            .ok();
    });

    // --- Spawn Orchestrator ---
    let gamma = GammaClient::new();
    let mut scheduler = Scheduler::new(
        gamma,
        storage_tx,
        broadcast_tx,
        epoch_tx,
        orch_state_tx,
        root_token.clone(),
    );

    let orch_token = root_token.clone();
    let _orch_handle = tokio::spawn(async move {
        scheduler.run().await;
    });

    // --- Wait for shutdown signal ---
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Received SIGINT, shutting down...");
        }
        _ = orch_token.cancelled() => {}
    }

    root_token.cancel();

    // Give tasks time to drain
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    tracing::info!("poly-collect shutdown complete");
    Ok(())
}
