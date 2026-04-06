# Data Usage Guide

How to read, group, and display the data from poly-collect.

## The Market

Every 5 minutes a new Polymarket market opens. Traders bet whether BTC will finish **above** (UP) or **below** (DOWN) the opening price when the 5-minute window expires.

- The **strike** is the first BTC trade price when the market opens — this is the price to beat.
- The **tau** counts down from 300 to 0 as the market approaches expiry.
- The **epoch** (unix timestamp divisible by 300) identifies which market window the data belongs to.

---

## Data Sources

| Source | What it is | Update rate |
|--------|-----------|-------------|
| `calculation` | Derived indicators, fair value, MAs | 1/sec |
| `binance_trade` | Raw BTC/USDT trade executions | Per trade (~10-50/sec) |
| `binance_depth` | BTC/USDT top-20 orderbook | Every 100ms |
| `polymarket_clob` | Polymarket UP/DOWN token orderbooks | Per event |

---

## Field Reference

### Calculation fields

| Field | What it means | Range |
|-------|--------------|-------|
| `btc_price` | Latest BTC/USDT trade price from Binance | ~60k-100k |
| `strike` | Opening BTC price of this market — the "price to beat" | Same as btc_price at market open, then fixed |
| `ma_7s` | Simple moving average over last 7 seconds | Tracks btc_price closely |
| `ma_25s` | Simple moving average over last 25 seconds | Smoother, lags more |
| `ma_99s` | Simple moving average over last 99 seconds | Smoothest, shows the trend |
| `rsi_14` | Relative Strength Index (14-period) | 0-100 (>70 overbought, <30 oversold) |
| `volatility` | Realized volatility per sqrt-second (300s rolling window) | Tiny number, ~0.00001 |
| `fair_value_up` | GBM model probability that BTC finishes above strike | 0.00-1.00 |
| `fair_value_down` | GBM model probability that BTC finishes below strike | 0.00-1.00 |
| `tau` | Seconds remaining until market expiry | 300 → 0 |
| `depth_imbalance` | BTC orderbook pressure: positive = more bids, negative = more asks | -1.0 to +1.0 |
| `mid_price` | BTC mid price: (best_bid + best_ask) / 2 | ~btc_price |
| `best_bid` | Highest BTC buy order | Just below btc_price |
| `best_ask` | Lowest BTC sell order | Just above btc_price |

### Binance trade fields

| Field | Key | What it means |
|-------|-----|--------------|
| Price | `p` | Trade execution price (string, full precision) |
| Quantity | `q` | Trade size in BTC |
| Trade time | `T` | When the trade happened (unix ms) |
| Side | `m` | `true` = seller was aggressive (sell), `false` = buyer was aggressive (buy) |

### Binance depth fields

| Field | What it means |
|-------|--------------|
| `bids` | Buy orders: `[[price, qty], ...]` highest first |
| `asks` | Sell orders: `[[price, qty], ...]` lowest first |

### Polymarket CLOB fields

| Field | What it means |
|-------|--------------|
| `asset_id` | Token ID (maps to UP or DOWN) |
| `bids` | Buy orders for the token: `[{price, size}, ...]` |
| `asks` | Sell orders for the token: `[{price, size}, ...]` |
| `best_bid` / `best_ask` | Top-of-book prices |

---

## How to Group the Data

### Group 1: BTC Price Chart

The primary chart. Shows BTC price movement against the strike line.

**Fields to display together:**
- `btc_price` — line chart (main series)
- `strike` — horizontal line (the target)
- `ma_7s` — fast MA overlay (reacts quickly to price changes)
- `ma_25s` — medium MA overlay
- `ma_99s` — slow MA overlay (shows the broader trend within the window)

**What to look for:**
- Price crossing above/below the strike tells you which side (UP/DOWN) is winning
- MA crossovers signal momentum shifts: when `ma_7s` crosses above `ma_25s`, short-term momentum is bullish
- All three MAs converging means low directional conviction — the market could go either way
- Price far from strike with low tau = high confidence in outcome

### Group 2: Fair Value vs Market Price (UP token)

Compare what the model says UP is worth vs what the market is pricing it at.

**Fields to display together:**
- `fair_value_up` — model's probability (0.00-1.00)
- Polymarket UP token `best_ask` — cheapest price to buy UP
- Polymarket UP token `best_bid` — highest price to sell UP

**What to look for:**
- `fair_value_up > best_ask` = UP token is **underpriced** (potential buy signal)
- `fair_value_up < best_bid` = UP token is **overpriced** (potential sell signal)
- The gap between fair value and market price is the **edge**
- Large edge = market disagrees with model or is slow to react

### Group 3: Fair Value vs Market Price (DOWN token)

Same concept, opposite side.

**Fields to display together:**
- `fair_value_down` — model's probability
- Polymarket DOWN token `best_ask` — cheapest price to buy DOWN
- Polymarket DOWN token `best_bid` — highest price to sell DOWN

**What to look for:**
- Same logic as UP but inverted. `fair_value_up + fair_value_down = 1.00`
- If UP is underpriced, DOWN is overpriced (and vice versa)
- Compare edge on both sides to find the better trade

### Group 4: Indicators Panel

Secondary indicators that provide context for the price chart.

**Fields to display together:**
- `rsi_14` — gauge or line (0-100)
- `depth_imbalance` — bar or gauge (-1.0 to +1.0)
- `tau` — countdown timer
- `volatility` — current realized vol

**What to look for:**
- RSI > 70 + price above strike = strong UP momentum, but potentially overbought
- RSI < 30 + price below strike = strong DOWN momentum, but potentially oversold
- Positive `depth_imbalance` = more BTC buy pressure on Binance (bullish bias)
- Negative `depth_imbalance` = more sell pressure (bearish bias)
- High volatility + high tau = fair value closer to 0.50 (uncertain outcome)
- Low volatility + low tau = fair value pushed toward extremes (more certain outcome)

### Group 5: BTC Orderbook

The Binance depth snapshot — shows where BTC liquidity sits.

**Fields to display together:**
- `binance_depth.bids` — buy wall (green side)
- `binance_depth.asks` — sell wall (red side)
- `mid_price` — center reference
- `depth_imbalance` — summary metric

**What to look for:**
- Large bid walls below price = support level, harder for price to fall
- Large ask walls above price = resistance, harder for price to rise
- Sudden wall removal can signal an imminent move

### Group 6: Polymarket Orderbooks

The UP and DOWN token books on Polymarket — shows where prediction market liquidity sits.

**Fields to display together:**
- UP token bids/asks
- DOWN token bids/asks
- `fair_value_up` / `fair_value_down` as reference lines

**What to look for:**
- Spread width: tight spread = liquid market, wide spread = illiquid
- Total size at best bid/ask: thicker = more conviction from market makers
- Compare against fair value to spot mispricing

---

## Relationships Between Fields

### Strike determines everything

```
btc_price > strike  →  UP is winning  →  fair_value_up > 0.50
btc_price < strike  →  DOWN is winning  →  fair_value_down > 0.50
btc_price = strike  →  coin flip  →  fair_value_up ≈ 0.50
```

### Time amplifies certainty

As `tau` decreases (market approaches expiry):
- If `btc_price` is above strike → `fair_value_up` accelerates toward 1.00
- If `btc_price` is below strike → `fair_value_up` accelerates toward 0.00
- Small price moves matter more with less time remaining

### Volatility dampens certainty

High `volatility` means the outcome is less certain:
- Even if price is above strike, high vol means it could easily cross back
- Fair value stays closer to 0.50 when volatility is high
- As the market ages and tau drops, vol matters less — the time effect dominates

### The edge equation

```
edge_up  = fair_value_up  - polymarket_up_best_ask
edge_down = fair_value_down - polymarket_down_best_ask
```

Positive edge means the model thinks the token is cheap relative to its true probability. The size of the edge (and whether it persists) determines if there's an actionable opportunity.

---

## Display Layout Suggestion

```
┌─────────────────────────────────────┬──────────────────────────┐
│                                     │                          │
│  BTC Price Chart                    │  Binance Depth           │
│  btc_price + strike + MAs           │  bids / asks             │
│  (Group 1)                          │  (Group 5)               │
│                                     │                          │
├──────────────────┬──────────────────┼──────────────────────────┤
│                  │                  │                          │
│  UP Token        │  DOWN Token      │  UP Book / DOWN Book     │
│  fair_value_up   │  fair_value_down │  Polymarket orderbooks   │
│  vs best_ask     │  vs best_ask     │  (Group 6)               │
│  (Group 2)       │  (Group 3)       │                          │
│                  │                  │                          │
├──────────────────┴──────────────────┴──────────────────────────┤
│  Indicators: RSI | Depth Imbalance | Tau Countdown | Vol       │
│  (Group 4)                                                     │
└────────────────────────────────────────────────────────────────┘
```

The top row is the primary view — BTC price action and liquidity. The middle row connects the BTC movement to the prediction market via fair value. The bottom row provides supporting context.
