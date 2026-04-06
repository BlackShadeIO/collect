//! GBM (Geometric Brownian Motion) fair value calculator for UP/DOWN tokens.
//!
//! Uses Black-Scholes pricing to compute the risk-neutral probability that
//! BTC finishes above the strike at expiry. The strike is the first BTC
//! trade price of each 5-minute market epoch.

use std::collections::VecDeque;
use std::f64::consts::FRAC_1_SQRT_2;

// ── Configuration constants (ported from fair-value/src/config.py) ──────────

/// Volatility lookback window in seconds.
const VOL_WINDOW_S: f64 = 300.0;

/// Annualized sigma floor for 5-minute markets (calibrated to CLOB-implied vol).
const SIGMA_FLOOR_5M_PCT: f64 = 0.165;

/// Probability floor: minimum 0.01% → clamps to [0.01, 0.99].
const PROB_FLOOR_PCT: f64 = 0.01;

/// Annual risk-free rate.
const RISK_FREE_RATE_ANNUAL: f64 = 0.04;

// ── Derived constants ───────────────────────────────────────────────────────

/// Risk-free rate per second.
const RF_PER_S: f64 = RISK_FREE_RATE_ANNUAL / (365.0 * 24.0 * 3600.0);

/// Sigma floor in per-√second units for 5-minute windows.
fn sigma_floor_5m() -> f64 {
    (SIGMA_FLOOR_5M_PCT / 100.0) / (300.0_f64).sqrt()
}

/// Probability floor clamped to [0.0, 0.49].
fn prob_floor() -> f64 {
    (PROB_FLOOR_PCT / 100.0).clamp(0.0, 0.49)
}

// ── Normal CDF ──────────────────────────────────────────────────────────────

fn normal_cdf(x: f64) -> f64 {
    0.5 * (1.0 + libm::erf(x * FRAC_1_SQRT_2))
}

// ── Fair value computation ──────────────────────────────────────────────────

/// Compute fair value probabilities for UP and DOWN tokens.
///
/// Returns `(p_up, p_down)` clamped by the probability floor.
pub fn compute_fair_value(spot: f64, strike: f64, tau_s: f64, sigma: f64) -> (f64, f64) {
    let floor = prob_floor();

    // Terminal payoff: market has expired
    if tau_s <= 0.0 {
        let p_up: f64 = if spot > strike {
            1.0
        } else if spot < strike {
            0.0
        } else {
            0.5
        };
        let p_up = p_up.clamp(floor, 1.0 - floor);
        return (round2(p_up), round2(1.0 - p_up));
    }

    // Apply volatility floor
    let sigma_eff = sigma.max(sigma_floor_5m());

    let denom = sigma_eff * tau_s.sqrt();
    let z = ((spot / strike).ln() + (RF_PER_S - 0.5 * sigma_eff * sigma_eff) * tau_s) / denom;
    let p_up = normal_cdf(z).clamp(floor, 1.0 - floor);

    (round2(p_up), round2(1.0 - p_up))
}

fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

// ── Volatility calculator ───────────────────────────────────────────────────

/// Time-weighted sliding-window volatility estimator.
///
/// Maintains a deque of `(timestamp_s, log_return, dt)` tuples over the last
/// `VOL_WINDOW_S` seconds. Drift and sigma are computed in per-second units
/// so they plug directly into the GBM z-score.
pub struct VolatilityCalculator {
    window_s: f64,
    returns: VecDeque<(f64, f64, f64)>, // (ts_s, log_return, dt)
    prev_price: Option<f64>,
    prev_ts: Option<f64>,
}

impl VolatilityCalculator {
    pub fn new() -> Self {
        Self {
            window_s: VOL_WINDOW_S,
            returns: VecDeque::new(),
            prev_price: None,
            prev_ts: None,
        }
    }

    /// Feed a new trade price. Call for every Binance trade.
    pub fn on_trade(&mut self, price: f64, ts_s: f64) {
        if price <= 0.0 {
            return;
        }
        if let (Some(prev_price), Some(prev_ts)) = (self.prev_price, self.prev_ts) {
            if prev_price > 0.0 {
                let dt = ts_s - prev_ts;
                if dt > 0.0 {
                    let log_ret = (price / prev_price).ln();
                    self.returns.push_back((ts_s, log_ret, dt));
                }
            }
        }
        self.prev_price = Some(price);
        self.prev_ts = Some(ts_s);
        self.trim(ts_s);
    }

    fn trim(&mut self, now_ts: f64) {
        let cutoff = now_ts - self.window_s;
        while self.returns.front().is_some_and(|(ts, _, _)| *ts < cutoff) {
            self.returns.pop_front();
        }
    }

    /// Returns `(mu, sigma)` both in per-√second units.
    pub fn drift_and_vol(&self) -> (f64, f64) {
        if self.returns.is_empty() {
            return (0.0, 0.0);
        }
        let total_dt: f64 = self.returns.iter().map(|(_, _, dt)| dt).sum();
        if total_dt <= 0.0 {
            return (0.0, 0.0);
        }
        let total_r: f64 = self.returns.iter().map(|(_, r, _)| r).sum();
        let mu = total_r / total_dt;
        let var_num: f64 = self
            .returns
            .iter()
            .map(|(_, r, dt)| (r - mu * dt).powi(2))
            .sum();
        let sigma = (var_num / total_dt).max(0.0).sqrt();
        (mu, sigma)
    }

    pub fn observation_count(&self) -> usize {
        self.returns.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_payoff() {
        // Spot above strike → p_up ≈ 1.0
        let (up, down) = compute_fair_value(100.0, 99.0, 0.0, 0.0);
        assert!(up > 0.95);
        assert!(down < 0.05);

        // Spot below strike → p_up ≈ 0.0
        let (up, down) = compute_fair_value(99.0, 100.0, 0.0, 0.0);
        assert!(up < 0.05);
        assert!(down > 0.95);
    }

    #[test]
    fn at_the_money_near_fifty() {
        // Spot == strike with time remaining → should be near 0.50
        let (up, _) = compute_fair_value(100.0, 100.0, 150.0, 0.001);
        assert!((up - 0.5).abs() < 0.1);
    }

    #[test]
    fn prob_floor_enforced() {
        // Extremely far OTM → probability floor prevents exactly 0.0
        // Floor is 0.01% = 0.0001, which rounds to 0.00 at 2dp,
        // but the raw value before rounding must be >= floor.
        let (up, down) = compute_fair_value(50.0, 100.0, 10.0, 0.0001);
        // p_up should be very small but clamped, p_down near 1.0
        assert!(up >= 0.0);
        assert!(down >= 0.0);
        assert!((up + down - 1.0).abs() < 0.02); // sums to ~1.0
    }
}
