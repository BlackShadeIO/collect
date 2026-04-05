#![allow(dead_code)] // Types used in integration tests and as the typed schema for stored data

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A price level on the Polymarket order book.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct PriceLevel {
    pub price: String,
    pub size: String,
}

/// An entry in the price_changes array of a price_change event.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PriceChangeEntry {
    pub asset_id: String,
    #[serde(default)]
    pub price: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Events received on the Polymarket CLOB market WebSocket channel.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "event_type")]
pub enum MarketEvent {
    /// Full order book snapshot.
    #[serde(rename = "book")]
    Book {
        asset_id: String,
        #[serde(default)]
        market: Option<String>,
        #[serde(default)]
        bids: Vec<PriceLevel>,
        #[serde(default)]
        asks: Vec<PriceLevel>,
        #[serde(default)]
        hash: Option<String>,
        #[serde(default)]
        timestamp: Option<String>,
        #[serde(flatten)]
        extra: HashMap<String, Value>,
    },

    /// Price change notification. Contains an array of per-asset price changes.
    #[serde(rename = "price_change")]
    PriceChange {
        #[serde(default)]
        market: Option<String>,
        #[serde(default)]
        price_changes: Vec<PriceChangeEntry>,
        #[serde(default)]
        timestamp: Option<String>,
        #[serde(flatten)]
        extra: HashMap<String, Value>,
    },

    /// Most recent trade price for an asset.
    #[serde(rename = "last_trade_price")]
    LastTradePrice {
        #[serde(default)]
        asset_id: Option<String>,
        #[serde(default)]
        market: Option<String>,
        #[serde(default)]
        price: Option<String>,
        #[serde(default)]
        size: Option<String>,
        #[serde(default)]
        side: Option<String>,
        #[serde(default)]
        fee_rate_bps: Option<String>,
        #[serde(default)]
        transaction_hash: Option<String>,
        #[serde(default)]
        timestamp: Option<String>,
        #[serde(flatten)]
        extra: HashMap<String, Value>,
    },

    /// Tick size change notification.
    #[serde(rename = "tick_size_change")]
    TickSizeChange {
        #[serde(default)]
        asset_id: Option<String>,
        #[serde(default)]
        new_tick_size: Option<String>,
        #[serde(default)]
        timestamp: Option<String>,
        #[serde(flatten)]
        extra: HashMap<String, Value>,
    },

    /// Best bid/ask snapshot (requires custom_feature_enabled).
    #[serde(rename = "best_bid_ask")]
    BestBidAsk {
        #[serde(default)]
        asset_id: Option<String>,
        #[serde(default)]
        market: Option<String>,
        #[serde(default)]
        best_bid: Option<String>,
        #[serde(default)]
        best_ask: Option<String>,
        #[serde(default)]
        spread: Option<String>,
        #[serde(default)]
        timestamp: Option<String>,
        #[serde(flatten)]
        extra: HashMap<String, Value>,
    },
}
