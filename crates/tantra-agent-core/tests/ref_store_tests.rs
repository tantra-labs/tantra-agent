//! Integration tests for `DataReferenceStore` (the MCP gateway reference
//! export/resolve store).
//!
//! Moved out of `src/ref_store.rs` per the Parallel Test Directory Rule
//! (`tantra-agent/docs/plans/0008`, gap 0001) — all tests use the public API.

use tantra_agent_core::DataReferenceStore;

#[test]
fn export_and_resolve_round_trip() {
    let store = DataReferenceStore::new();
    let uri = store.export(
        "ohlcv_reliance_5m",
        serde_json::json!({"instrument": "NSE:RELIANCE-EQ", "close": "2500.00"}),
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
    let uri = store.export("key", serde_json::json!({"v": 1}));
    assert_eq!(store.len(), 1);
    store.export("key", serde_json::json!({"v": 2}));
    assert_eq!(store.len(), 1);
    assert_eq!(store.resolve(&uri).unwrap()["v"], 2);
}

#[test]
fn remove_and_uris() {
    let store = DataReferenceStore::new();
    store.export("a", serde_json::json!({"v": 1}));
    store.export("b", serde_json::json!({"v": 2}));
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
