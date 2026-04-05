//! Phase 2: Type validation tests.
//! Deserialize research samples into typed structs and verify round-trip.

use poly_collect::types::binance::{BinanceDepthSnapshot, BinanceTrade};
use poly_collect::types::gamma::{GammaEvent, current_epoch, next_epoch, seconds_remaining, slug_for_epoch};
use poly_collect::types::polymarket::MarketEvent;

#[test]
fn test_binance_trade_serde() {
    let json = r#"{"e":"trade","E":1775371640129,"s":"BTCUSDT","t":6184636124,"p":"66826.58000000","q":"0.00022000","T":1775371640128,"m":true,"M":true}"#;

    let trade: BinanceTrade = serde_json::from_str(json).expect("deserialize BinanceTrade");
    assert_eq!(trade.event_type, "trade");
    assert_eq!(trade.event_time, 1775371640129);
    assert_eq!(trade.symbol, "BTCUSDT");
    assert_eq!(trade.trade_id, 6184636124);
    assert_eq!(trade.price, "66826.58000000");
    assert_eq!(trade.quantity, "0.00022000");
    assert_eq!(trade.trade_time, 1775371640128);
    assert!(trade.buyer_is_maker);
    assert!(trade.best_match);
    assert!(trade.extra.is_empty());

    // Round-trip
    let reserialized = serde_json::to_string(&trade).unwrap();
    let trade2: BinanceTrade = serde_json::from_str(&reserialized).unwrap();
    assert_eq!(trade.price, trade2.price);
    assert_eq!(trade.trade_id, trade2.trade_id);
}

#[test]
fn test_binance_depth_snapshot_serde() {
    let json = r#"{"lastUpdateId":91435111660,"bids":[["66826.58000000","2.18168000"],["66826.57000000","0.00040000"]],"asks":[["66826.59000000","0.64093000"],["66826.60000000","0.00032000"]]}"#;

    let snap: BinanceDepthSnapshot = serde_json::from_str(json).expect("deserialize BinanceDepthSnapshot");
    assert_eq!(snap.last_update_id, 91435111660);
    assert_eq!(snap.bids.len(), 2);
    assert_eq!(snap.bids[0][0], "66826.58000000");
    assert_eq!(snap.bids[0][1], "2.18168000");
    assert_eq!(snap.asks.len(), 2);
    assert_eq!(snap.asks[0][0], "66826.59000000");
    assert!(snap.extra.is_empty());

    // Round-trip
    let reserialized = serde_json::to_string(&snap).unwrap();
    let snap2: BinanceDepthSnapshot = serde_json::from_str(&reserialized).unwrap();
    assert_eq!(snap.last_update_id, snap2.last_update_id);
    assert_eq!(snap.bids, snap2.bids);
}

#[test]
fn test_polymarket_book_event_serde() {
    let json = r#"{"event_type":"book","asset_id":"80164095529621317955631598180798953178189130110174773574033462280074845413443","market":"0xe0046f7314a5ad8b96ac23b9740976c8c176aa4a8dadbf44d14e9b698859d1cc","bids":[{"price":"0.01","size":"7876.48"}],"asks":[{"price":"0.99","size":"8871.28"}],"hash":"9a3cdf68fbf70163d3a30cba914c1a041cb0d5bd","timestamp":"1775371646136"}"#;

    let event: MarketEvent = serde_json::from_str(json).expect("deserialize Book event");
    match &event {
        MarketEvent::Book { asset_id, market, bids, asks, hash, timestamp, .. } => {
            assert!(asset_id.starts_with("801640"));
            assert!(market.is_some());
            assert_eq!(bids.len(), 1);
            assert_eq!(bids[0].price, "0.01");
            assert_eq!(asks.len(), 1);
            assert_eq!(asks[0].price, "0.99");
            assert_eq!(hash.as_deref(), Some("9a3cdf68fbf70163d3a30cba914c1a041cb0d5bd"));
            assert_eq!(timestamp.as_deref(), Some("1775371646136"));
        }
        _ => panic!("expected Book variant"),
    }

    // Round-trip
    let reserialized = serde_json::to_string(&event).unwrap();
    let _event2: MarketEvent = serde_json::from_str(&reserialized).unwrap();
}

#[test]
fn test_polymarket_price_change_serde() {
    let json = r#"{"event_type":"price_change","market":"0xe0046f7314a5ad8b96ac23b9740976c8c176aa4a8dadbf44d14e9b698859d1cc","price_changes":[{"asset_id":"80164095529621317955631598180798953178189130110174773574033462280074845413443","price":"0.505"}],"timestamp":"1775371639915"}"#;

    let event: MarketEvent = serde_json::from_str(json).expect("deserialize PriceChange event");
    match &event {
        MarketEvent::PriceChange { market, price_changes, timestamp, .. } => {
            assert!(market.is_some());
            assert_eq!(price_changes.len(), 1);
            assert_eq!(price_changes[0].price.as_deref(), Some("0.505"));
            assert!(price_changes[0].asset_id.starts_with("801640"));
            assert_eq!(timestamp.as_deref(), Some("1775371639915"));
        }
        _ => panic!("expected PriceChange variant"),
    }
}

#[test]
fn test_polymarket_last_trade_price_serde() {
    let json = r#"{"event_type":"last_trade_price","asset_id":"81156883204798280453543796672727175092331521578479358374426633074950228393879","market":"0xe0046f7314a5ad8b96ac23b9740976c8c176aa4a8dadbf44d14e9b698859d1cc","price":"0.5","size":"52","side":"BUY","fee_rate_bps":"1000","transaction_hash":"0x261779034df939bb074d7a405d097b619fe47fe7530bd2fc5d3196e01acf1cce","timestamp":"1775371646152"}"#;

    let event: MarketEvent = serde_json::from_str(json).expect("deserialize LastTradePrice event");
    match &event {
        MarketEvent::LastTradePrice { asset_id, price, size, side, fee_rate_bps, transaction_hash, .. } => {
            assert!(asset_id.is_some());
            assert_eq!(price.as_deref(), Some("0.5"));
            assert_eq!(size.as_deref(), Some("52"));
            assert_eq!(side.as_deref(), Some("BUY"));
            assert_eq!(fee_rate_bps.as_deref(), Some("1000"));
            assert!(transaction_hash.is_some());
        }
        _ => panic!("expected LastTradePrice variant"),
    }
}

#[test]
fn test_polymarket_best_bid_ask_serde() {
    let json = r#"{"event_type":"best_bid_ask","asset_id":"80164095529621317955631598180798953178189130110174773574033462280074845413443","market":"0xe0046f7314a5ad8b96ac23b9740976c8c176aa4a8dadbf44d14e9b698859d1cc","best_bid":"0.5","best_ask":"0.51","spread":"0.01","timestamp":"1775371646138"}"#;

    let event: MarketEvent = serde_json::from_str(json).expect("deserialize BestBidAsk event");
    match &event {
        MarketEvent::BestBidAsk { best_bid, best_ask, spread, .. } => {
            assert_eq!(best_bid.as_deref(), Some("0.5"));
            assert_eq!(best_ask.as_deref(), Some("0.51"));
            assert_eq!(spread.as_deref(), Some("0.01"));
        }
        _ => panic!("expected BestBidAsk variant"),
    }
}

#[test]
fn test_gamma_event_serde() {
    // Minimal test with actual structure discovered
    let json = r#"{"id":"342253","slug":"btc-updown-5m-1775371800","title":"Bitcoin Up or Down","markets":[{"conditionId":"0xe004","question":"BTC?","clobTokenIds":"[\"token1\", \"token2\"]","active":true}],"active":true}"#;

    let event: GammaEvent = serde_json::from_str(json).expect("deserialize GammaEvent");
    assert_eq!(event.slug.as_deref(), Some("btc-updown-5m-1775371800"));
    assert_eq!(event.active, Some(true));
    let markets = event.markets.unwrap();
    assert_eq!(markets.len(), 1);
    assert_eq!(markets[0].condition_id, "0xe004");
    assert!(markets[0].extra.contains_key("clobTokenIds"));
}

#[test]
fn test_epoch_helpers() {
    let epoch = 1775371500u64;
    assert_eq!(slug_for_epoch(epoch), "btc-updown-5m-1775371500");
    assert_eq!(next_epoch(epoch), 1775371800);

    let cur = current_epoch();
    assert_eq!(cur % 300, 0);

    let rem = seconds_remaining(cur);
    assert!(rem > 0 && rem <= 300);
}

#[test]
fn test_binance_trade_with_extra_fields() {
    // If Binance adds new fields, they should be captured
    let json = r#"{"e":"trade","E":1000,"s":"BTCUSDT","t":1,"p":"100","q":"1","T":1000,"m":true,"M":true,"newField":"surprise"}"#;
    let trade: BinanceTrade = serde_json::from_str(json).unwrap();
    assert_eq!(trade.extra.get("newField").unwrap(), "surprise");
}
