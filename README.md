# poly-collect

Real-time data collector for Polymarket BTC 5-minute prediction markets. Concurrently streams data from Polymarket CLOB WebSocket and Binance WebSocket, computes live technical indicators, persists everything as JSONL, and exposes an authenticated REST API + WebSocket stream.

## What it collects

- **Polymarket CLOB** — Orderbook snapshots, price changes, trades for each 5-minute BTC Up/Down market
- **Binance Trades** — BTC/USDT trade executions
- **Binance Depth** — Top-20 orderbook snapshots (100ms interval)
- **Calculations** — EMA-9, EMA-21, RSI-14, depth imbalance, mid price

Data is organized per market epoch (5-minute windows) in `data/{epoch}/`.

## Requirements

- Rust 1.88.0+
- Internet access (Polymarket & Binance WebSocket APIs)

## Setup

```bash
git clone https://github.com/BlackShadeIO/collect.git
cd collect

cp .env.example .env
# Edit .env — set your API_KEY at minimum
```

### .env configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `API_KEY` | `changeme` | Bearer token for API authentication |
| `API_HOST` | `0.0.0.0` | API server bind address |
| `API_PORT` | `8080` | API server port |
| `DATA_DIR` | `./data` | Where JSONL files are stored |
| `LOG_LEVEL` | `info` | Tracing log level (`debug`, `info`, `warn`, `error`) |

## Run

```bash
cargo run
```

The collector will:
1. Start the API server
2. Connect to Binance trade + depth WebSocket streams
3. Discover the current BTC 5-minute market via Polymarket's Gamma API
4. Subscribe to the Polymarket CLOB WebSocket for that market's orderbook
5. Compute technical indicators from the Binance trade stream
6. Automatically rotate to the next market every 5 minutes

Stop with `Ctrl+C` — all data is flushed before shutdown.

## API

See [API.md](API.md) for full endpoint documentation.

Quick reference:

```bash
# Health check (no auth)
curl http://localhost:8080/health

# Live status
curl -H "Authorization: Bearer $API_KEY" http://localhost:8080/status

# Storage stats
curl -H "Authorization: Bearer $API_KEY" http://localhost:8080/stats

# List collected markets
curl -H "Authorization: Bearer $API_KEY" http://localhost:8080/market

# Download data for a specific market
curl -H "Authorization: Bearer $API_KEY" \
  "http://localhost:8080/download?epoch=1775394000&category=binance_trade" -o trades.jsonl

# Live WebSocket stream
websocat "ws://localhost:8080/ws?token=$API_KEY"
```

## Tests

```bash
cargo test
```

## Data layout

```
data/
  {epoch}/
    binance_trade.jsonl
    binance_depth.jsonl
    polymarket_clob.jsonl
    calculation.jsonl
```

Each line is a JSON `StorageRecord`:

```json
{
  "ts": 1775394000777,
  "source": "binance_trade",
  "epoch": 1775394000,
  "data": { "e": "trade", "p": "66826.58", "q": "0.001", ... }
}
```
