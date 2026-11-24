# `tantra-agent` — Data Reference Store Reference

Documents `tantra-agent-core/src/ref_store.rs`: the ephemeral in-memory data reference
store emitting `$tantra/ref/<uuid>` URIs, plus the [`RefReader`] port and chart-series
normalizer.

## Purpose

To prevent huge token costs when chaining market quotes into technicals and portfolio
optimization, large time-series are cached here by value and only a compact reference URI is
returned to the caller. Downstream tools (or chat hosts rendering `DisplayBlock::Chart` via
`RefReader`) resolve the reference back to the payload or a bounded series slice.

## `DataReferenceStore` & `RefReader`

```rust
pub const MAX_CHART_SERIES_POINTS: usize = 500;

pub trait RefReader: Send + Sync {
    fn resolve(&self, reference_uri: &str) -> Option<Value>;
}

pub struct DataReferenceStore {
    inner: Arc<RwLock<HashMap<String, RefEntry>>>,
}
```

### Methods & Functions

| Symbol | Signature | Behavior |
|--------|-----------|----------|
| `new` | `() -> Self` | Empty store |
| `export` | `(reference_id, data: Value) -> String` | Persists payload, returns `$tantra/ref/<reference_id>` |
| `resolve` | `(&reference_uri) -> Option<Value>` | Rehydrates a previously exported payload (also exposed via `impl RefReader for DataReferenceStore`) |
| `len` | `() -> usize` | Cached entry count |
| `is_empty` | `() -> bool` | True when no entries |
| `remove` | `(&reference_uri) -> bool` | Remove entry by URI |
| `uris` | `() -> Vec<String>` | All cached reference URIs (discovery/introspection) |
| `extract_chart_series` | `(value: &Value, max_points: usize) -> Vec<(i64, String)>` | Pure, zero-float `Decimal`-normalizing extractor handling `[ts, val]` tuples, object shapes, scalar arrays, wrapper keys (`series`/`points`/`bars`/`candles`/`equity_curve`/`data`), unix/RFC3339 timestamps, and endpoint-preserving uniform downsampling |

## `RefEntry`

| Field | Type | Meaning |
|-------|------|---------|
| `uri` | `String` | Full `$tantra/ref/<reference_id>` URI |
| `data` | `serde_json::Value` | Cached payload |
| `size_bytes` | `usize` | Serialised payload size (telemetry / cap checks) |
| `created_at` | `DateTime<Utc>` | Creation timestamp |

## Zero-Floats Note

All stored values are arbitrary `serde_json::Value`s; any `Decimal`-derived numbers are
serialised to the model as JSON strings so precision survives the wire (matching the
`tantra-agent-core::result` precision contract `"exact_decimal_zero_floats"`).
`extract_chart_series` parses numeric values through `rust_decimal::Decimal` and emits
canonical decimal strings without `f32`/`f64` intermediates.

## Related

- `tantra-agent-core/tests/ref_store_tests.rs` — unit/contract tests for store + `RefReader` + `extract_chart_series`.
- `tantra-agent-core/src/result.rs` — `TantraResultEnvelope.refs` holds these URIs; `DisplayBlock::Chart.series` carries resolved bounded slices.
- `tantra-agent-mcp/src/server.rs` — `McpServer.data_store: DataReferenceStore`.
