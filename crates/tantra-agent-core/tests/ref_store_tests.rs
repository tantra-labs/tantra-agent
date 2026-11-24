//! Integration tests for `DataReferenceStore` (the MCP gateway reference
//! export/resolve store).
//!
//! Moved out of `src/ref_store.rs` per the Parallel Test Directory Rule
//! (`tantra-agent/docs/plans/0008`, gap 0001) — all tests use the public API.

use serde_json::json;
use tantra_agent_core::{
    DataReferenceStore, DisplayBlock, MAX_CHART_SERIES_POINTS, RefReader, TantraResultEnvelope,
    extract_chart_series,
};

#[test]
fn export_and_resolve_round_trip() {
    let store = DataReferenceStore::new();
    let uri = store.export(
        "ohlcv_reliance_5m",
        json!({"instrument": "NSE:RELIANCE-EQ", "close": "2500.00"}),
    );
    assert_eq!(uri, "$tantra/ref/ohlcv_reliance_5m");
    assert!(store.resolve(&uri).is_some());
    let resolved = store.resolve(&uri).unwrap();
    assert_eq!(resolved["instrument"], "NSE:RELIANCE-EQ");
    assert_eq!(resolved["close"], "2500.00");
}

#[test]
fn unknown_reference_resolves_to_none() {
    let store = DataReferenceStore::new();
    assert!(store.resolve("$tantra/ref/does-not-exist").is_none());
    assert!(store.is_empty());
}

#[test]
fn export_overwrites_same_id() {
    let store = DataReferenceStore::new();
    let uri = store.export("key", json!({"v": 1}));
    assert_eq!(store.len(), 1);
    store.export("key", json!({"v": 2}));
    assert_eq!(store.len(), 1);
    assert_eq!(store.resolve(&uri).unwrap()["v"], 2);
}

#[test]
fn remove_and_uris() {
    let store = DataReferenceStore::new();
    store.export("a", json!({"v": 1}));
    store.export("b", json!({"v": 2}));
    assert_eq!(store.len(), 2);
    let mut uris = store.uris();
    uris.sort();
    assert_eq!(
        uris,
        vec!["$tantra/ref/a".to_string(), "$tantra/ref/b".to_string()]
    );
    assert!(store.remove("$tantra/ref/a"));
    assert_eq!(store.len(), 1);
    assert!(store.resolve("$tantra/ref/a").is_none());
    assert!(store.resolve("$tantra/ref/b").is_some());
}

#[test]
fn ref_reader_trait_resolves_from_data_reference_store() {
    let store = DataReferenceStore::new();
    let uri = store.export(
        "equity_001",
        json!([[1_700_000_000, "100.00"], [1_700_003_600, "102.50"]]),
    );
    let reader: &dyn RefReader = &store;
    let payload = reader.resolve(&uri).expect("should resolve via RefReader");
    let series = extract_chart_series(&payload, MAX_CHART_SERIES_POINTS);
    assert_eq!(
        series,
        vec![
            (1_700_000_000, "100.00".to_string()),
            (1_700_003_600, "102.50".to_string()),
        ]
    );
}

#[test]
fn extract_chart_series_handles_objects_wrappers_and_bounding() {
    let wrapped = json!({
        "series": [
            {"timestamp": "2026-11-20T09:15:00Z", "close": "2450.25"},
            {"timestamp": 1_700_000_100, "value": 2451.5}
        ]
    });
    let pts = extract_chart_series(&wrapped, MAX_CHART_SERIES_POINTS);
    assert_eq!(pts.len(), 2);
    assert_eq!(pts[0].1, "2450.25");
    assert_eq!(pts[1], (1_700_000_100, "2451.5".to_string()));

    // Downsample > MAX_CHART_SERIES_POINTS preserving first & last endpoints.
    let raw: Vec<serde_json::Value> = (0..1200)
        .map(|i| json!([1_700_000_000_i64 + i, format!("{}.00", 100 + i)]))
        .collect();
    let bounded = extract_chart_series(&json!(raw), MAX_CHART_SERIES_POINTS);
    assert_eq!(bounded.len(), MAX_CHART_SERIES_POINTS);
    assert_eq!(bounded.first().unwrap(), &(1_700_000_000, "100.00".into()));
    assert_eq!(bounded.last().unwrap(), &(1_700_001_199, "1299.00".into()));
}

#[test]
fn chart_display_block_series_round_trips_and_defaults_cleanly() {
    let minimal: DisplayBlock = serde_json::from_value(json!({
        "type": "chart",
        "title": "Equity Curve",
        "chart_type": "line",
        "series_ref": "$tantra/ref/eq-1"
    }))
    .expect("deserialize legacy chart block");
    assert!(matches!(
        &minimal,
        DisplayBlock::Chart { series, .. } if series.is_empty()
    ));

    let populated = DisplayBlock::Chart {
        title: "Equity Curve".into(),
        chart_type: "line".into(),
        series_ref: "$tantra/ref/eq-1".into(),
        series: vec![(100, "1000.00".into()), (200, "1050.25".into())],
    };
    let env = TantraResultEnvelope::ok("Chart ready", json!({})).with_block(populated);
    let encoded = serde_json::to_value(&env).unwrap();
    assert_eq!(encoded["blocks"][0]["series"][1][0], 200);
    assert_eq!(encoded["blocks"][0]["series"][1][1], "1050.25");
    let decoded: TantraResultEnvelope<serde_json::Value> = serde_json::from_value(encoded).unwrap();
    assert!(matches!(
        decoded.blocks.as_slice(),
        [DisplayBlock::Chart { series, .. }] if series.len() == 2 && series[0].1 == "1000.00"
    ));
}
