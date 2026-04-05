use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

const INTERVAL: u64 = 300;

// ---------------------------------------------------------------------------
// Epoch helpers
// ---------------------------------------------------------------------------

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_secs()
}

pub fn current_epoch() -> u64 {
    let now = now_unix();
    now - (now % INTERVAL)
}

pub fn next_epoch(epoch: u64) -> u64 {
    epoch + INTERVAL
}

pub fn seconds_remaining(epoch: u64) -> i64 {
    let end = epoch + INTERVAL;
    end as i64 - now_unix() as i64
}

pub fn slug_for_epoch(epoch: u64) -> String {
    format!("btc-updown-5m-{epoch}")
}

// ---------------------------------------------------------------------------
// Gamma API response types
// ---------------------------------------------------------------------------

/// A market as returned by the Gamma API.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GammaMarket {
    pub condition_id: String,
    #[serde(default)]
    pub question_id: Option<String>,
    #[serde(default)]
    pub question: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub market_slug: Option<String>,
    #[serde(default)]
    pub end_date_iso: Option<String>,
    #[serde(default)]
    pub game_start_time: Option<String>,
    #[serde(default)]
    pub tokens: Option<Vec<GammaToken>>,
    #[serde(default)]
    pub minimum_tick_size: Option<f64>,
    #[serde(default)]
    pub neg_risk: Option<bool>,
    #[serde(default)]
    pub active: Option<bool>,
    #[serde(default)]
    pub closed: Option<bool>,
    #[serde(default)]
    pub accepting_orders: Option<bool>,
    #[serde(default)]
    pub volume: Option<String>,
    #[serde(default)]
    pub liquidity: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// An outcome token within a Gamma market.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GammaToken {
    pub token_id: String,
    pub outcome: String,
    #[serde(default)]
    pub price: Option<f64>,
    #[serde(default)]
    pub winner: Option<bool>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// An event as returned by the Gamma API.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GammaEvent {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub markets: Option<Vec<GammaMarket>>,
    #[serde(default)]
    pub start_date: Option<String>,
    #[serde(default)]
    pub end_date: Option<String>,
    #[serde(default)]
    pub active: Option<bool>,
    #[serde(default)]
    pub closed: Option<bool>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Resolved BTC 5-minute market with extracted IDs.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct BtcMarket {
    pub epoch: u64,
    pub slug: String,
    pub question: String,
    pub condition_id: String,
    pub yes_token_id: String,
    pub no_token_id: String,
}
