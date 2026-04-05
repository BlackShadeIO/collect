//! Gamma API client for discovering BTC 5-minute markets.

use anyhow::{Context, Result, bail};
use tracing::{info, warn};

use crate::types::gamma::{BtcMarket, GammaEvent, slug_for_epoch};

const GAMMA_BASE_URL: &str = "https://gamma-api.polymarket.com";

#[derive(Clone)]
pub struct GammaClient {
    client: reqwest::Client,
}

impl GammaClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    /// Find a BTC 5-minute market for the given epoch.
    pub async fn find_market(&self, epoch: u64) -> Result<BtcMarket> {
        let slug = slug_for_epoch(epoch);
        info!(slug = %slug, "Looking up BTC 5m market");

        let url = format!("{GAMMA_BASE_URL}/events?slug={slug}&limit=1");
        let resp = self.client.get(&url).send().await.context("Gamma API request failed")?;
        let status = resp.status();
        let text = resp.text().await.context("Failed to read Gamma response")?;

        if !status.is_success() {
            bail!("Gamma API returned {status}: {text}");
        }

        let events: Vec<GammaEvent> = serde_json::from_str(&text)
            .context("Failed to parse Gamma events")?;

        let event = events
            .into_iter()
            .next()
            .with_context(|| format!("No event found for slug {slug}"))?;

        let markets = event.markets
            .with_context(|| format!("No markets in event {slug}"))?;
        let market = markets
            .into_iter()
            .next()
            .with_context(|| format!("Empty markets array for {slug}"))?;

        // clobTokenIds is a JSON-encoded string containing an array
        let clob_token_ids_raw = market.extra.get("clobTokenIds")
            .with_context(|| format!("No clobTokenIds for {slug}"))?;

        let token_ids: Vec<String> = if let Some(s) = clob_token_ids_raw.as_str() {
            serde_json::from_str(s)
                .with_context(|| format!("Failed to parse clobTokenIds string: {s}"))?
        } else if let Some(arr) = clob_token_ids_raw.as_array() {
            arr.iter().filter_map(|v| v.as_str().map(String::from)).collect()
        } else {
            bail!("clobTokenIds is neither string nor array: {clob_token_ids_raw}");
        };

        if token_ids.len() < 2 {
            bail!("Expected 2 token IDs for {slug}, got {}", token_ids.len());
        }

        Ok(BtcMarket {
            epoch,
            slug,
            question: market.question.unwrap_or_default(),
            condition_id: market.condition_id,
            yes_token_id: token_ids[0].clone(),
            no_token_id: token_ids[1].clone(),
        })
    }

    /// Find market with retries and exponential backoff.
    pub async fn find_market_with_retry(&self, epoch: u64, max_retries: u32) -> Result<BtcMarket> {
        let mut delay = std::time::Duration::from_secs(2);
        for attempt in 0..=max_retries {
            match self.find_market(epoch).await {
                Ok(market) => return Ok(market),
                Err(e) => {
                    if attempt == max_retries {
                        return Err(e);
                    }
                    warn!(attempt, error = %e, "Gamma API failed, retrying in {:?}", delay);
                    tokio::time::sleep(delay).await;
                    delay *= 2;
                }
            }
        }
        unreachable!()
    }
}
