//! Phase 1: Research & Discovery tests.
//! These tests connect to live APIs and WebSockets to capture real data structures.
//! Run with: cargo test --test research -- --nocapture

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio_tungstenite::{connect_async, tungstenite::Message};

// ---------------------------------------------------------------------------
// 1A: Gamma API — discover 5-minute BTC markets
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_gamma_api_discovery() {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let current_epoch = now - (now % 300);
    let next_epoch = current_epoch + 300;
    let slug = format!("btc-updown-5m-{next_epoch}");

    println!("=== Gamma API Discovery ===");
    println!("Current epoch: {current_epoch}");
    println!("Next epoch: {next_epoch}");
    println!("Slug: {slug}");

    let client = reqwest::Client::new();
    let url = format!(
        "https://gamma-api.polymarket.com/events?slug={slug}&limit=1"
    );
    println!("Request URL: {url}");

    let resp = client.get(&url).send().await.expect("Gamma API request failed");
    let status = resp.status();
    let text = resp.text().await.expect("Failed to read response body");

    println!("Status: {status}");
    println!("Response body:\n{text}");

    // Save to research directory
    let path = format!("research/gamma_responses/events_{next_epoch}.json");
    std::fs::write(&path, &text).expect("Failed to save gamma response");
    println!("Saved to {path}");

    // Parse and validate
    let events: Vec<Value> = serde_json::from_str(&text).expect("Failed to parse as JSON array");

    if events.is_empty() {
        // Try current epoch instead — market may already be live
        let slug2 = format!("btc-updown-5m-{current_epoch}");
        let url2 = format!(
            "https://gamma-api.polymarket.com/events?slug={slug2}&limit=1"
        );
        println!("\nNo results for next epoch, trying current: {url2}");
        let resp2 = client.get(&url2).send().await.expect("Gamma API request failed");
        let text2 = resp2.text().await.expect("Failed to read response body");
        println!("Response body:\n{text2}");

        let path2 = format!("research/gamma_responses/events_{current_epoch}.json");
        std::fs::write(&path2, &text2).expect("Failed to save gamma response");

        let events2: Vec<Value> = serde_json::from_str(&text2).expect("Failed to parse");
        assert!(!events2.is_empty(), "No events found for either epoch");

        validate_gamma_event(&events2[0]);
    } else {
        validate_gamma_event(&events[0]);
    }
}

fn validate_gamma_event(event: &Value) {
    println!("\n=== Validating Gamma Event ===");
    println!("Event slug: {:?}", event["slug"]);
    println!("Event title: {:?}", event["title"]);

    let markets = event["markets"].as_array().expect("markets should be an array");
    assert!(!markets.is_empty(), "Event should have at least one market");

    let market = &markets[0];
    println!("Market conditionId: {:?}", market["conditionId"]);
    println!("Market question: {:?}", market["question"]);
    println!("Market clobTokenIds: {:?}", market["clobTokenIds"]);
    println!("Market tokens: {:?}", market["tokens"]);
    println!("Market active: {:?}", market["active"]);
    println!("Market closed: {:?}", market["closed"]);

    // Verify required fields exist
    assert!(market["conditionId"].is_string(), "conditionId should be a string");

    // clobTokenIds might be a JSON string containing an array
    let token_ids_raw = &market["clobTokenIds"];
    let token_ids: Vec<String> = if let Some(s) = token_ids_raw.as_str() {
        serde_json::from_str(s).unwrap_or_else(|_| vec![s.to_string()])
    } else if let Some(arr) = token_ids_raw.as_array() {
        arr.iter().filter_map(|v| v.as_str().map(String::from)).collect()
    } else {
        panic!("clobTokenIds should be a string or array, got: {token_ids_raw}");
    };
    println!("Parsed token IDs: {token_ids:?}");
    assert!(token_ids.len() >= 2, "Should have at least 2 token IDs (Yes/No)");

    println!("\nFull market JSON (all fields):");
    println!("{}", serde_json::to_string_pretty(market).unwrap());
}

// ---------------------------------------------------------------------------
// 1B: Binance Trade WebSocket
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_binance_trade_ws() {
    println!("=== Binance Trade WebSocket ===");

    let url = "wss://stream.binance.com:9443/ws/btcusdt@trade";
    let (mut ws, _) = connect_async(url).await.expect("Failed to connect to Binance trade WS");

    let mut messages = Vec::new();
    let timeout = tokio::time::sleep(Duration::from_secs(15));
    tokio::pin!(timeout);

    loop {
        tokio::select! {
            msg = ws.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        let text_str: &str = &text;
                        println!("Trade msg #{}: {text_str}", messages.len() + 1);
                        let parsed: Value = serde_json::from_str(text_str)
                            .expect("Trade message should be valid JSON");
                        messages.push(parsed);
                        if messages.len() >= 20 {
                            break;
                        }
                    }
                    Some(Ok(Message::Ping(data))) => {
                        let _ = ws.send(Message::Pong(data)).await;
                    }
                    Some(Err(e)) => panic!("WS error: {e}"),
                    _ => {}
                }
            }
            _ = &mut timeout => {
                println!("Timeout after 15s, collected {} messages", messages.len());
                break;
            }
        }
    }

    assert!(messages.len() >= 5, "Should receive at least 5 trade messages");

    // Save samples
    let lines: Vec<String> = messages.iter().map(|m| m.to_string()).collect();
    std::fs::write("research/binance_trade_samples.jsonl", lines.join("\n"))
        .expect("Failed to save trade samples");

    // Validate fields
    let first = &messages[0];
    assert_eq!(first["e"].as_str(), Some("trade"), "event_type should be 'trade'");
    assert!(first["E"].is_i64() || first["E"].is_u64(), "E should be integer");
    assert!(first["s"].is_string(), "s should be string");
    assert!(first["t"].is_i64() || first["t"].is_u64(), "t should be integer");
    assert!(first["p"].is_string(), "p (price) should be STRING");
    assert!(first["q"].is_string(), "q (quantity) should be STRING");
    assert!(first["T"].is_i64() || first["T"].is_u64(), "T should be integer");
    assert!(first["m"].is_boolean(), "m should be boolean");
    assert!(first["M"].is_boolean(), "M should be boolean");

    println!("\nAll fields in first message:");
    for (k, v) in first.as_object().unwrap() {
        println!("  {k}: {} ({})", v, value_type(v));
    }

    let _ = ws.close(None).await;
}

// ---------------------------------------------------------------------------
// 1C: Binance Depth WebSocket
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_binance_depth_ws() {
    println!("=== Binance Depth WebSocket ===");

    // First fetch REST snapshot
    let client = reqwest::Client::new();
    let snap_resp = client
        .get("https://api.binance.com/api/v3/depth?symbol=BTCUSDT&limit=20")
        .send()
        .await
        .expect("Failed to fetch depth snapshot");
    let snap_text = snap_resp.text().await.expect("Failed to read snapshot body");
    println!("REST snapshot (first 500 chars): {}", &snap_text[..snap_text.len().min(500)]);

    std::fs::write("research/binance_depth_snapshot.json", &snap_text)
        .expect("Failed to save depth snapshot");

    let snapshot: Value = serde_json::from_str(&snap_text).expect("Snapshot should be valid JSON");
    assert!(snapshot["lastUpdateId"].is_i64() || snapshot["lastUpdateId"].is_u64());
    assert!(snapshot["bids"].is_array());
    assert!(snapshot["asks"].is_array());

    // Now connect to depth stream
    let url = "wss://stream.binance.com:9443/ws/btcusdt@depth20@100ms";
    let (mut ws, _) = connect_async(url).await.expect("Failed to connect to depth WS");

    let mut messages = Vec::new();
    let timeout = tokio::time::sleep(Duration::from_secs(15));
    tokio::pin!(timeout);

    loop {
        tokio::select! {
            msg = ws.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        let text_str: &str = &text;
                        let parsed: Value = serde_json::from_str(text_str)
                            .expect("Depth message should be valid JSON");
                        if messages.len() < 3 {
                            println!("Depth msg #{}: {text_str}", messages.len() + 1);
                        }
                        messages.push(parsed);
                        if messages.len() >= 20 {
                            break;
                        }
                    }
                    Some(Ok(Message::Ping(data))) => {
                        let _ = ws.send(Message::Pong(data)).await;
                    }
                    Some(Err(e)) => panic!("WS error: {e}"),
                    _ => {}
                }
            }
            _ = &mut timeout => {
                println!("Timeout after 15s, collected {} messages", messages.len());
                break;
            }
        }
    }

    assert!(messages.len() >= 5, "Should receive at least 5 depth messages");

    let lines: Vec<String> = messages.iter().map(|m| m.to_string()).collect();
    std::fs::write("research/binance_depth_samples.jsonl", lines.join("\n"))
        .expect("Failed to save depth samples");

    // Validate first message — depth20@100ms sends partial snapshots
    let first = &messages[0];
    println!("\nAll fields in first depth message:");
    for (k, v) in first.as_object().unwrap() {
        println!("  {k}: {} ({})", truncate_value(v), value_type(v));
    }
    assert!(first["lastUpdateId"].is_i64() || first["lastUpdateId"].is_u64(),
        "lastUpdateId should be present");
    assert!(first["bids"].is_array(), "bids should be array");
    assert!(first["asks"].is_array(), "asks should be array");

    let _ = ws.close(None).await;
}

// ---------------------------------------------------------------------------
// 1D: Polymarket CLOB WebSocket
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_polymarket_clob_ws() {
    println!("=== Polymarket CLOB WebSocket ===");

    // First get a token ID from Gamma
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let current_epoch = now - (now % 300);
    let next_epoch = current_epoch + 300;

    let client = reqwest::Client::new();

    // Try next epoch, then current
    let mut token_ids: Vec<String> = Vec::new();
    for epoch in [next_epoch, current_epoch] {
        let slug = format!("btc-updown-5m-{epoch}");
        let url = format!("https://gamma-api.polymarket.com/events?slug={slug}&limit=1");
        let resp = client.get(&url).send().await.expect("Gamma request failed");
        let text = resp.text().await.unwrap();
        let events: Vec<Value> = serde_json::from_str(&text).unwrap_or_default();

        if let Some(event) = events.first() {
            if let Some(markets) = event["markets"].as_array() {
                if let Some(market) = markets.first() {
                    let raw = &market["clobTokenIds"];
                    if let Some(s) = raw.as_str() {
                        token_ids = serde_json::from_str(s).unwrap_or_default();
                    } else if let Some(arr) = raw.as_array() {
                        token_ids = arr.iter().filter_map(|v| v.as_str().map(String::from)).collect();
                    }
                    if !token_ids.is_empty() {
                        println!("Found token IDs from epoch {epoch}: {token_ids:?}");
                        break;
                    }
                }
            }
        }
    }

    assert!(!token_ids.is_empty(), "Failed to find any token IDs from Gamma API");

    // Connect to Polymarket CLOB WebSocket
    let ws_url = "wss://ws-subscriptions-clob.polymarket.com/ws/market";
    let (mut ws, _) = connect_async(ws_url).await.expect("Failed to connect to Polymarket WS");

    // Subscribe
    let subscribe_msg = serde_json::json!({
        "assets_ids": token_ids,
        "type": "market",
        "custom_feature_enabled": true,
    });
    println!("Subscribing with: {subscribe_msg}");
    ws.send(Message::Text(subscribe_msg.to_string().into())).await.expect("Failed to send subscribe");

    // Set up heartbeat
    let mut messages = Vec::new();
    let mut ping_interval = tokio::time::interval(Duration::from_secs(10));
    ping_interval.tick().await; // consume first tick

    let timeout = tokio::time::sleep(Duration::from_secs(60));
    tokio::pin!(timeout);

    loop {
        tokio::select! {
            _ = ping_interval.tick() => {
                println!("Sending PING heartbeat");
                let _ = ws.send(Message::Text("PING".into())).await;
            }
            msg = ws.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        let text_str: &str = &text;
                        if text_str == "PONG" {
                            println!("Received PONG");
                            continue;
                        }
                        println!("CLOB msg #{}: {}", messages.len() + 1,
                            &text_str[..text_str.len().min(200)]);
                        match serde_json::from_str::<Value>(text_str) {
                            Ok(parsed) => {
                                messages.push(parsed);
                            }
                            Err(e) => {
                                println!("  (not JSON: {e})");
                            }
                        }
                        if messages.len() >= 20 {
                            break;
                        }
                    }
                    Some(Ok(Message::Ping(data))) => {
                        let _ = ws.send(Message::Pong(data)).await;
                    }
                    Some(Err(e)) => {
                        println!("WS error: {e}");
                        break;
                    }
                    None => {
                        println!("WS stream ended");
                        break;
                    }
                    _ => {}
                }
            }
            _ = &mut timeout => {
                println!("Timeout after 60s, collected {} messages", messages.len());
                break;
            }
        }
    }

    // Save samples
    if !messages.is_empty() {
        let lines: Vec<String> = messages.iter().map(|m| m.to_string()).collect();
        std::fs::write("research/polymarket_clob_samples.jsonl", lines.join("\n"))
            .expect("Failed to save CLOB samples");
    }

    assert!(!messages.is_empty(), "Should receive at least 1 CLOB message");

    // Validate message types
    for (i, msg) in messages.iter().enumerate() {
        if let Some(event_type) = msg["event_type"].as_str() {
            println!("\nMessage {i} event_type: {event_type}");
            println!("  All fields:");
            if let Some(obj) = msg.as_object() {
                for (k, v) in obj {
                    println!("    {k}: {} ({})", truncate_value(v), value_type(v));
                }
            }
        }
    }

    let _ = ws.close(None).await;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn value_type(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(n) => {
            if n.is_i64() { "i64" }
            else if n.is_u64() { "u64" }
            else { "f64" }
        }
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn truncate_value(v: &Value) -> String {
    let s = v.to_string();
    if s.len() > 80 {
        format!("{}...", &s[..77])
    } else {
        s
    }
}
