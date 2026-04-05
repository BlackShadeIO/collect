# poly-collect API Reference

Base URL: `http://localhost:8080` (configurable via `API_HOST` and `API_PORT` in `.env`)

## Authentication

All endpoints except `/health` require a Bearer token set via the `API_KEY` environment variable.

```
Authorization: Bearer <API_KEY>
```

The WebSocket endpoint authenticates via query parameter instead (see below).

---

## REST Endpoints

### GET /health

System health check. **No authentication required.**

```bash
curl http://localhost:8080/health
```

```json
{
  "status": "ok",
  "uptime_secs": 342,
  "current_epoch": 1775393700,
  "active_processes": 2
}
```

---

### GET /status

Live orchestrator state and latest technical indicators.

```bash
curl -H "Authorization: Bearer $API_KEY" http://localhost:8080/status
```

```json
{
  "orchestrator": {
    "current_epoch": 1775393700,
    "slots": [
      {
        "slot": 0,
        "epoch": 1775393700,
        "slug": "btc-updown-5m-1775393700",
        "state": "Live",
        "seconds_remaining": 187
      },
      {
        "slot": 1,
        "epoch": 1775394000,
        "slug": "btc-updown-5m-1775394000",
        "state": "PreMarket",
        "seconds_remaining": 487
      }
    ]
  },
  "latest_indicator": {
    "ts": 1775393813000,
    "epoch": 1775393700,
    "btc_price": 66826.59,
    "ema_9": 66825.12,
    "ema_21": 66824.87,
    "rsi_14": 52.3,
    "depth_imbalance": 0.034,
    "mid_price": 66826.585,
    "best_bid": 66826.58,
    "best_ask": 66826.59
  }
}
```

**Process states:** `PreMarket` | `Live` | `Draining` | `Done`

---

### GET /stats

Aggregate storage statistics across all collected data.

```bash
curl -H "Authorization: Bearer $API_KEY" http://localhost:8080/stats
```

```json
{
  "uptime_secs": 620,
  "total_files": 12,
  "total_size_bytes": 4823901,
  "categories": {
    "binance_trade": { "file_count": 3, "total_size": 2105332 },
    "binance_depth": { "file_count": 3, "total_size": 1893421 },
    "polymarket_clob": { "file_count": 3, "total_size": 712004 },
    "calculation": { "file_count": 3, "total_size": 113144 }
  }
}
```

---

### GET /market

List all market epochs that have collected data on disk.

```bash
curl -H "Authorization: Bearer $API_KEY" http://localhost:8080/market
```

```json
{
  "markets": [
    {
      "epoch": 1775393400,
      "slug": "btc-updown-5m-1775393400",
      "total_size_bytes": 1520443,
      "files": [
        { "name": "binance_trade.jsonl", "size_bytes": 680122 },
        { "name": "binance_depth.jsonl", "size_bytes": 612301 },
        { "name": "polymarket_clob.jsonl", "size_bytes": 185020 },
        { "name": "calculation.jsonl", "size_bytes": 43000 }
      ]
    }
  ]
}
```

---

### GET /market/{epoch}

Detailed info for a specific market epoch, including per-file line counts.

```bash
curl -H "Authorization: Bearer $API_KEY" http://localhost:8080/market/1775393400
```

```json
{
  "epoch": 1775393400,
  "slug": "btc-updown-5m-1775393400",
  "total_size_bytes": 1520443,
  "total_lines": 8234,
  "files": [
    { "name": "binance_trade.jsonl", "size_bytes": 680122, "line_count": 4102 },
    { "name": "binance_depth.jsonl", "size_bytes": 612301, "line_count": 3000 },
    { "name": "polymarket_clob.jsonl", "size_bytes": 185020, "line_count": 832 },
    { "name": "calculation.jsonl", "size_bytes": 43000, "line_count": 300 }
  ]
}
```

Returns `404` if the epoch directory does not exist.

---

### GET /download

Download collected JSONL data. Returns concatenated NDJSON (`application/x-ndjson`).

| Parameter  | Type   | Required | Description |
|------------|--------|----------|-------------|
| `epoch`    | u64    | No       | Filter to a specific market epoch |
| `category` | string | No       | Filter to a data source |

**Categories:** `binance_trade`, `binance_depth`, `polymarket_clob`, `calculation`

```bash
# All data for a specific market
curl -H "Authorization: Bearer $API_KEY" \
  "http://localhost:8080/download?epoch=1775393400"

# Only Binance trades across all markets
curl -H "Authorization: Bearer $API_KEY" \
  "http://localhost:8080/download?category=binance_trade"

# Polymarket data for one market
curl -H "Authorization: Bearer $API_KEY" \
  "http://localhost:8080/download?epoch=1775393400&category=polymarket_clob"

# Save to file
curl -H "Authorization: Bearer $API_KEY" \
  "http://localhost:8080/download?epoch=1775393400" -o market_data.jsonl
```

Returns `404` if no matching files are found.

Each line is a `StorageRecord`:

```json
{"ts":1775393813000,"source":"binance_trade","epoch":1775393700,"data":{"e":"trade","E":1775393813000,"s":"BTCUSDT","t":6184636124,"p":"66826.58000000","q":"0.00022000","T":1775393812999,"m":true,"M":true}}
```

---

## WebSocket Stream

### GET /ws?token=API_KEY

Live streaming of all collected data over WebSocket. Authenticates via the `token` query parameter.

#### Connect

```bash
# Using websocat
websocat "ws://localhost:8080/ws?token=$API_KEY"

# Using wscat
wscat -c "ws://localhost:8080/ws?token=$API_KEY"
```

#### Messages Received

Each message is a JSON `StorageRecord`:

```json
{
  "ts": 1775393813000,
  "source": "binance_trade",
  "epoch": 1775393700,
  "data": { ... }
}
```

The `source` field tells you the data type:

| Source | Content |
|--------|---------|
| `binance_trade` | BTC/USDT trade executions |
| `binance_depth` | Top-20 orderbook snapshots (100ms) |
| `polymarket_clob` | Polymarket orderbook events |
| `calculation` | Technical indicators (EMA, RSI, depth imbalance) |

#### Filtering

By default all sources are streamed. Send JSON messages to filter:

**Subscribe to specific sources only:**
```json
{"subscribe": ["binance_trade", "calculation"]}
```

**Unsubscribe from sources:**
```json
{"unsubscribe": ["binance_depth"]}
```

**Example: only receive calculations:**
```json
{"unsubscribe": ["binance_trade", "binance_depth", "polymarket_clob"]}
```

#### Calculation Data Shape

When `source` is `"calculation"`, the `data` field contains:

```json
{
  "ts": 1775393813000,
  "epoch": 1775393700,
  "btc_price": 66826.59,
  "ema_9": 66825.12,
  "ema_21": 66824.87,
  "rsi_14": 52.3,
  "depth_imbalance": 0.034,
  "mid_price": 66826.585,
  "best_bid": 66826.58,
  "best_ask": 66826.59
}
```

| Field | Description |
|-------|-------------|
| `btc_price` | Latest BTC/USDT trade price |
| `ema_9` | 9-period Exponential Moving Average |
| `ema_21` | 21-period Exponential Moving Average |
| `rsi_14` | 14-period Relative Strength Index (0-100) |
| `depth_imbalance` | `(bid_qty - ask_qty) / (bid_qty + ask_qty)`, range [-1, 1] |
| `mid_price` | `(best_bid + best_ask) / 2` |

---

## Error Responses

| Status | Meaning |
|--------|---------|
| `401 Unauthorized` | Missing or invalid `Authorization` header / `token` param |
| `404 Not Found` | Requested epoch or data does not exist |
| `500 Internal Server Error` | Server error |

---

## Data Storage Layout

All collected data is stored on disk as JSONL files:

```
data/
  {epoch}/
    binance_trade.jsonl
    binance_depth.jsonl
    polymarket_clob.jsonl
    calculation.jsonl
```

Each epoch corresponds to a 5-minute BTC market window (unix timestamp divisible by 300).
