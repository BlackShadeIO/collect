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

### GET /snapshot

Latest indicator and depth state in a single call. Lighter than `/status` for clients that only need current market data.

```bash
curl -H "Authorization: Bearer $API_KEY" http://localhost:8080/snapshot
```

```json
{
  "current_epoch": 1775393700,
  "indicator": {
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
  },
  "depth": {
    "best_bid": 66826.58,
    "best_ask": 66826.59,
    "total_bid_qty": 12.345,
    "total_ask_qty": 11.890,
    "mid_price": 66826.585
  }
}
```

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
| `from`     | i64    | No       | Minimum timestamp, unix milliseconds (inclusive) |
| `to`       | i64    | No       | Maximum timestamp, unix milliseconds (inclusive) |

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

# Last 2 minutes of calculation data for the current epoch
curl -H "Authorization: Bearer $API_KEY" \
  "http://localhost:8080/download?epoch=1775393700&category=calculation&from=1775393573000"

# Time-range filter: only records between two timestamps
curl -H "Authorization: Bearer $API_KEY" \
  "http://localhost:8080/download?epoch=1775393400&from=1775393500000&to=1775393600000"

# Save to file
curl -H "Authorization: Bearer $API_KEY" \
  "http://localhost:8080/download?epoch=1775393400" -o market_data.jsonl
```

Returns `404` if no matching files are found (or no records match the time range).

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

#### Subscription Filtering

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

#### Backfill

After connecting, you can request recent historical data from disk. The server reads the JSONL files for the requested epoch, filters by time, sorts by timestamp, and sends matching records over the WebSocket before continuing with the live stream.

**Request backfill:**
```json
{"backfill": {"last_seconds": 120}}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `last_seconds` | u64 | `120` | How many seconds of history to send |
| `sources` | string[] | all four | Which sources to include |
| `epoch` | u64 | current | Specific epoch to backfill from |

**Examples:**

```json
// Last 2 minutes of all data for the current epoch
{"backfill": {"last_seconds": 120}}

// Last 60 seconds of calculations + CLOB data only
{"backfill": {"last_seconds": 60, "sources": ["calculation", "polymarket_clob"]}}

// Full backfill of a specific epoch (last 5 minutes = entire market)
{"backfill": {"last_seconds": 300, "epoch": 1775393400}}
```

**Backfill end marker:**

After all backfill records have been sent, the server sends a marker message:

```json
{"backfill_end": true, "count": 847}
```

Use this marker to know when backfill is complete and all subsequent messages are live data. The `count` field indicates how many records were sent during backfill.

**Combining backfill with subscribe/unsubscribe:**

You can send backfill and subscription commands in the same message or as separate messages:

```json
{"backfill": {"last_seconds": 120, "sources": ["calculation", "polymarket_clob"]}, "subscribe": ["calculation", "polymarket_clob"], "unsubscribe": ["binance_trade", "binance_depth"]}
```

---

## Data Shapes

### StorageRecord (wrapper)

Every record from both the WebSocket stream and `/download` endpoint uses this wrapper:

```json
{
  "ts": 1775393813000,
  "source": "binance_trade",
  "epoch": 1775393700,
  "data": { ... }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `ts` | i64 | Receive timestamp (unix milliseconds) |
| `source` | string | One of: `binance_trade`, `binance_depth`, `polymarket_clob`, `calculation` |
| `epoch` | u64 \| null | 5-minute market epoch (unix timestamp divisible by 300) |
| `data` | object | Source-specific payload (see below) |

### Calculation (source: `calculation`)

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
| `best_bid` | Current best bid price |
| `best_ask` | Current best ask price |

Emitted at most once per second.

### Binance Trade (source: `binance_trade`)

```json
{
  "e": "trade",
  "E": 1775393813000,
  "s": "BTCUSDT",
  "t": 6184636124,
  "p": "66826.58000000",
  "q": "0.00022000",
  "T": 1775393812999,
  "m": true,
  "M": true
}
```

| Field | Type | Description |
|-------|------|-------------|
| `e` | string | Event type (always `"trade"`) |
| `E` | i64 | Event time (unix ms) |
| `s` | string | Symbol (always `"BTCUSDT"`) |
| `t` | i64 | Trade ID |
| `p` | string | Price |
| `q` | string | Quantity |
| `T` | i64 | Trade time (unix ms) |
| `m` | bool | Buyer is market maker (`true` = sell, `false` = buy) |

### Binance Depth (source: `binance_depth`)

Top-20 orderbook snapshot, updated every 100ms.

```json
{
  "lastUpdateId": 12345678,
  "bids": [["66826.58", "0.123"], ["66826.57", "0.456"]],
  "asks": [["66826.59", "0.789"], ["66826.60", "0.321"]]
}
```

| Field | Type | Description |
|-------|------|-------------|
| `bids` | `[string, string][]` | `[price, quantity]` pairs, highest price first |
| `asks` | `[string, string][]` | `[price, quantity]` pairs, lowest price first |

### Polymarket CLOB (source: `polymarket_clob`)

Events from the Polymarket orderbook. Tagged by event type — common shapes include:

**Book snapshot (tokens array format):**
```json
{
  "event_type": "book",
  "market": "...",
  "timestamp": 1775393813,
  "tokens": [
    {
      "token_id": "abc123",
      "outcome": "UP",
      "bids": [{"price": "0.55", "size": "100"}],
      "asks": [{"price": "0.57", "size": "50"}]
    },
    {
      "token_id": "def456",
      "outcome": "DOWN",
      "bids": [{"price": "0.43", "size": "80"}],
      "asks": [{"price": "0.45", "size": "60"}]
    }
  ]
}
```

**Other event types:** `price_change`, `last_trade_price`, `tick_size_change`, `best_bid_ask`

### Depth State (from `/snapshot`)

Aggregated Binance depth summary:

```json
{
  "best_bid": 66826.58,
  "best_ask": 66826.59,
  "total_bid_qty": 12.345,
  "total_ask_qty": 11.890,
  "mid_price": 66826.585
}
```

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

---

## Terminal Integration Guide

This section documents how to integrate a web frontend (like [collect_terminal](https://github.com/BlackShadeIO/collect_terminal)) with the poly-collect API.

### Environment Setup

```env
NEXT_PUBLIC_API_BASE=http://100.123.193.72:8080
NEXT_PUBLIC_API_KEY=your-api-key
```

### Recommended Live Page Flow

The most efficient way to implement a live trading view:

#### Option A: WebSocket-only (recommended)

Connect to the WebSocket and use the backfill command to get recent history. This eliminates the separate HTTP round-trip that was previously required.

```typescript
const ws = new WebSocket(`ws://${API_BASE}/ws?token=${API_KEY}`);

ws.onopen = () => {
  // Request last 2 minutes of history, then seamlessly continue with live data
  ws.send(JSON.stringify({
    backfill: { last_seconds: 120 }
  }));
};

let backfilling = true;

ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);

  if (msg.backfill_end) {
    // All historical records have been received.
    // Everything after this point is live.
    backfilling = false;
    console.log(`Backfill complete: ${msg.count} records`);
    return;
  }

  // Same StorageRecord format for both backfill and live data
  const record: StorageRecord = msg;
  handleRecord(record, backfilling);
};
```

#### Option B: HTTP backfill + WebSocket live

Use the time-filtered `/download` endpoint for backfill, then switch to WebSocket:

```typescript
// 1. Get current epoch
const status = await fetchStatus();
const epoch = status.orchestrator?.current_epoch;

// 2. Download only the last 2 minutes (not the entire epoch)
const from = Date.now() - 120_000;
const response = await fetch(
  `${API_BASE}/download?epoch=${epoch}&from=${from}`,
  { headers: { Authorization: `Bearer ${API_KEY}` } }
);

// 3. Parse NDJSON stream — all records are already time-filtered server-side
for await (const record of parseNDJSONStream(response)) {
  addToStore(record);
}

// 4. Connect WebSocket for live data
connectWebSocket();
```

This is significantly faster than the previous approach of downloading the full epoch and filtering client-side.

### Polling Endpoints

For status indicators (bottom bar, connection status):

| Endpoint | Suggested Interval | Use Case |
|----------|-------------------|----------|
| `/health` | 10s | Connection indicator, uptime display |
| `/snapshot` | 5s | Latest indicator values, depth state, current epoch |
| `/stats` | 30s | Storage size, file counts |
| `/market` | 30s | Market list for explore page |

Use `/snapshot` instead of `/status` when you only need the latest indicator and depth data — it's lighter and returns the depth state that `/status` doesn't include.

### Data Flow Architecture

```
                    ┌─────────────────────────────┐
                    │       poly-collect           │
                    │                              │
                    │  ┌───────────┐ ┌──────────┐  │
                    │  │  JSONL    │ │ Broadcast│  │
                    │  │  Files    │ │ Channel  │  │
                    │  └─────┬─────┘ └────┬─────┘  │
                    │        │            │        │
                    │   ┌────┴──┐    ┌────┴────┐   │
                    │   │  REST │    │   WS    │   │
                    │   │  API  │    │  Stream │   │
                    │   └───┬───┘    └────┬────┘   │
                    └───────┼─────────────┼────────┘
                            │             │
              ┌─────────────┼─────────────┼──────────────┐
              │  Web Terminal│             │              │
              │             │             │              │
              │   ┌─────────▼──┐   ┌──────▼───────┐     │
              │   │ /snapshot  │   │  WebSocket   │     │
              │   │ /download  │   │  + backfill  │     │
              │   │ /market    │   │              │     │
              │   └─────┬──────┘   └──────┬───────┘     │
              │         │                 │             │
              │   ┌─────▼─────────────────▼──────┐     │
              │   │     Zustand Stores           │     │
              │   │  live | connection | replay  │     │
              │   └──────────────┬───────────────┘     │
              │                  │                     │
              │   ┌──────────────▼───────────────┐     │
              │   │  Charts, Orderbooks, Panels  │     │
              │   └──────────────────────────────┘     │
              └────────────────────────────────────────┘
```

### TypeScript Types

These types match the API response shapes:

```typescript
// Core record from WebSocket and /download
type DataSource = "binance_trade" | "binance_depth" | "polymarket_clob" | "calculation";

interface StorageRecord {
  ts: number;
  source: DataSource;
  epoch: number;
  data: BinanceTrade | BinanceDepthSnapshot | PolymarketClob | CalculationData;
}

// /snapshot response
interface SnapshotResponse {
  current_epoch: number;
  indicator: CalculationData;
  depth: {
    best_bid: number;
    best_ask: number;
    total_bid_qty: number;
    total_ask_qty: number;
    mid_price: number;
  };
}

// WebSocket backfill end marker
interface BackfillEnd {
  backfill_end: true;
  count: number;
}

// Discriminate between backfill markers and regular records
function isBackfillEnd(msg: unknown): msg is BackfillEnd {
  return typeof msg === "object" && msg !== null && "backfill_end" in msg;
}
```

### Explore/Replay Page

For the historical data explorer and replay mode:

```typescript
// 1. List available markets
const { markets } = await fetchMarkets();

// 2. Get details for a specific epoch
const detail = await fetchMarket(epoch);

// 3. Download full epoch data for replay
const response = await createDownloadStream({ epoch });
const records: StorageRecord[] = [];
for await (const record of parseNDJSONStream(response)) {
  records.push(record);
}

// 4. Sort by timestamp for deterministic replay
records.sort((a, b) => a.ts - b.ts);

// 5. Use a requestAnimationFrame loop to advance through records
//    based on elapsed time * playback speed
```

### WebSocket Reconnection

The WebSocket should automatically reconnect with exponential backoff. After reconnecting, re-send the backfill request to recover any missed data:

```typescript
ws.onopen = () => {
  reconnectAttempts = 0;
  // Re-request backfill to fill any gap from the disconnection
  ws.send(JSON.stringify({
    backfill: { last_seconds: 30 }
  }));
};
```
