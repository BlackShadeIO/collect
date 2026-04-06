pub mod binance;
pub mod gamma;
pub mod polymarket;

use serde::{Deserialize, Serialize};

/// Wrapper for all data persisted to JSONL files.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageRecord {
    /// Our receive timestamp (unix milliseconds).
    pub ts: i64,
    /// Source identifier: "polymarket_clob", "binance_trade", "binance_depth", "calculation".
    pub source: String,
    /// Associated market epoch (5-minute window start timestamp).
    pub epoch: Option<u64>,
    /// The full original message, preserving all fields.
    pub data: serde_json::Value,
}

/// Live technical indicator snapshot emitted by the calculation engine.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IndicatorSnapshot {
    pub ts: i64,
    pub epoch: u64,
    pub btc_price: f64,
    /// First BTC trade price of this market epoch (the price to beat).
    pub strike: f64,
    /// 7-second simple moving average.
    pub ma_7s: f64,
    /// 25-second simple moving average.
    pub ma_25s: f64,
    /// 99-second simple moving average.
    pub ma_99s: f64,
    pub rsi_14: f64,
    /// Realized volatility per √second (sliding 300s window).
    pub volatility: f64,
    /// Fair probability the UP token finishes in the money.
    pub fair_value_up: f64,
    /// Fair probability the DOWN token finishes in the money.
    pub fair_value_down: f64,
    /// Seconds remaining until market expiry.
    pub tau: f64,
    /// (total_bid_qty - total_ask_qty) / (total_bid_qty + total_ask_qty)
    pub depth_imbalance: f64,
    pub mid_price: f64,
    pub best_bid: f64,
    pub best_ask: f64,
}

/// Aggregated depth state maintained by the depth collector.
#[derive(Serialize, Debug, Clone, Default)]
pub struct DepthState {
    pub best_bid: f64,
    pub best_ask: f64,
    pub total_bid_qty: f64,
    pub total_ask_qty: f64,
    pub mid_price: f64,
}
