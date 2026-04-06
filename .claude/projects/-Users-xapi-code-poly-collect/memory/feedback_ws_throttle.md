---
name: WebSocket 100ms throttle
description: API WebSocket should throttle output to 100ms intervals to avoid overwhelming clients
type: feedback
---

API WebSocket stream should only push updates to clients every 100ms, not per-record. Raw broadcast rate is too high for web terminals.

**Why:** The underlying data sources (especially Binance trades and depth at 100ms) produce too much data for browser clients to handle in real-time.

**How to apply:** When modifying the WS stream handler, keep the 100ms flush interval. Buffer records between ticks and send them in batches.
