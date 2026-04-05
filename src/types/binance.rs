use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A single trade from the Binance `btcusdt@trade` stream.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BinanceTrade {
    /// Event type — always "trade".
    #[serde(rename = "e")]
    pub event_type: String,
    /// Event time (unix milliseconds).
    #[serde(rename = "E")]
    pub event_time: i64,
    /// Symbol (e.g. "BTCUSDT").
    #[serde(rename = "s")]
    pub symbol: String,
    /// Trade ID.
    #[serde(rename = "t")]
    pub trade_id: i64,
    /// Price (string to preserve precision).
    #[serde(rename = "p")]
    pub price: String,
    /// Quantity (string to preserve precision).
    #[serde(rename = "q")]
    pub quantity: String,
    /// Trade time (unix milliseconds).
    #[serde(rename = "T")]
    pub trade_time: i64,
    /// Is the buyer the market maker?
    #[serde(rename = "m")]
    pub buyer_is_maker: bool,
    /// Was this the best price match?
    #[serde(rename = "M")]
    pub best_match: bool,
    /// Catch-all for undocumented fields.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Partial depth snapshot from `btcusdt@depth20@100ms`.
/// Each message is a full snapshot of the top 20 levels.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BinanceDepthSnapshot {
    #[serde(rename = "lastUpdateId")]
    pub last_update_id: i64,
    /// Bid levels: [[price_str, qty_str], ...] highest first.
    pub bids: Vec<[String; 2]>,
    /// Ask levels: [[price_str, qty_str], ...] lowest first.
    pub asks: Vec<[String; 2]>,
    /// Catch-all for undocumented fields.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

