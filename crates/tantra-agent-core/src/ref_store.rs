//! Ephemeral in-memory data reference store emitting `$tantra/ref/<uuid>` URIs.
//!
//! To prevent huge token costs when chaining market quotes into technicals and
//! portfolio optimization, large time-series are cached here by value and only
//! a compact reference URI is returned to the caller. Downstream tools resolve
//! the reference back to the full payload.
//!
//! Zero-floats note: all stored values are arbitrary `serde_json::Value`s; any
//! `Decimal`-derived numbers are serialised to the model as JSON strings so
//! precision survives the wire (see `tantra-agent-core::result` precision
//! contract `"exact_decimal_zero_floats"`).

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Maximum number of `(timestamp_sec, decimal_string)` points attached inline
/// when resolving a `DisplayBlock::Chart` reference for chat clients.
pub const MAX_CHART_SERIES_POINTS: usize = 500;

/// Read-only port for resolving `$tantra/ref/...` URIs cached by tools.
pub trait RefReader: Send + Sync {
    /// Resolve a reference URI to its stored JSON payload, if present.
    fn resolve(&self, reference_uri: &str) -> Option<Value>;
}

/// A single cached reference entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefEntry {
    /// Full reference URI, e.g. `$tantra/ref/ohlcv_reliance_5m_20261106`.
    pub uri: String,
    /// Cached payload.
    pub data: Value,
    /// Size of the serialised payload in bytes (for telemetry / cap checks).
    pub size_bytes: usize,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

/// In-memory Arrow/SQLite-backed ephemeral cache keyed by
/// `$tantra/ref/<reference_id>`.
///
/// `export` persists a payload and returns its reference URI;
/// `resolve` rehydrates a previously-exported payload by URI.
#[derive(Debug, Default, Clone)]
pub struct DataReferenceStore {
    inner: Arc<RwLock<HashMap<String, RefEntry>>>,
}

impl DataReferenceStore {
    /// Create an empty reference store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Persist `data` under `reference_id` and return its reference URI
    /// (`$tantra/ref/<reference_id>`).
    pub fn export(&self, reference_id: impl Into<String>, data: Value) -> String {
        let reference_id = reference_id.into();
        let uri = format!("$tantra/ref/{reference_id}");
        let serialised = serde_json::to_string(&data).unwrap_or_default();
        let entry = RefEntry {
            uri: uri.clone(),
            data,
            size_bytes: serialised.len(),
            created_at: Utc::now(),
        };
        self.inner.write().unwrap().insert(uri.clone(), entry);
        uri
    }

    /// Rehydrate a previously exported payload by its reference URI.
    ///
    /// Returns `None` when the reference is unknown or has expired.
    pub fn resolve(&self, reference_uri: &str) -> Option<Value> {
        self.inner
            .read()
            .unwrap()
            .get(reference_uri)
            .map(|entry| entry.data.clone())
    }

    /// Return the number of entries currently cached.
    pub fn len(&self) -> usize {
        self.inner.read().unwrap().len()
    }

    /// Return `true` when no entries are cached.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Remove a reference by URI. Returns `true` if an entry was removed.
    pub fn remove(&self, reference_uri: &str) -> bool {
        self.inner.write().unwrap().remove(reference_uri).is_some()
    }

    /// Iterate over all cached reference URIs (for discovery / introspection).
    pub fn uris(&self) -> Vec<String> {
        self.inner.read().unwrap().keys().cloned().collect()
    }
}

impl RefReader for DataReferenceStore {
    fn resolve(&self, reference_uri: &str) -> Option<Value> {
        DataReferenceStore::resolve(self, reference_uri)
    }
}

impl FromIterator<(String, Value)> for DataReferenceStore {
    fn from_iter<T: IntoIterator<Item = (String, Value)>>(iter: T) -> Self {
        let store = DataReferenceStore::new();
        for (id, data) in iter {
            store.export(id, data);
        }
        store
    }
}

fn parse_decimal_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => {
            let trimmed = s.trim();
            if Decimal::from_str_exact(trimmed).is_ok() {
                Some(trimmed.to_string())
            } else {
                Decimal::from_scientific(trimmed)
                    .ok()
                    .map(|d| d.normalize().to_string())
            }
        }
        Value::Number(n) => {
            let raw = n.to_string();
            if Decimal::from_str_exact(&raw).is_ok() {
                Some(raw)
            } else {
                Decimal::from_scientific(&raw)
                    .ok()
                    .map(|d| d.normalize().to_string())
            }
        }
        _ => None,
    }
}

fn parse_timestamp_sec(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => {
            let trimmed = s.trim();
            if let Ok(sec) = trimmed.parse::<i64>() {
                Some(sec)
            } else {
                DateTime::parse_from_rfc3339(trimmed)
                    .ok()
                    .map(|dt| dt.timestamp())
            }
        }
        _ => None,
    }
}

fn unwrap_series_array(value: &Value) -> Option<&Vec<Value>> {
    if let Some(arr) = value.as_array() {
        return Some(arr);
    }
    let obj = value.as_object()?;
    for key in [
        "series",
        "points",
        "bars",
        "candles",
        "equity_curve",
        "data",
    ] {
        if let Some(inner) = obj.get(key) {
            if let Some(arr) = inner.as_array() {
                return Some(arr);
            }
            if let Some(arr) = unwrap_series_array(inner) {
                return Some(arr);
            }
        }
    }
    None
}

/// Extract a bounded `(timestamp_sec, decimal_string)` chart series from a
/// stored JSON reference payload.
///
/// Accepts:
/// - Arrays of `[timestamp, value]` tuples (`timestamp` unix seconds or RFC3339 string)
/// - Arrays of objects (`timestamp`/`ts`/`time`/`t`/`x` + `close`/`value`/`price`/`equity`/`v`/`y`)
/// - Arrays of scalar decimal strings or numbers (indexed `0..N`)
/// - Wrapper objects containing `series`, `points`, `bars`, `candles`, `equity_curve`, or `data`
///
/// When more than `max_points` valid points exist, the series is uniformly
/// downsampled while preserving the chronological order and both endpoints.
pub fn extract_chart_series(value: &Value, max_points: usize) -> Vec<(i64, String)> {
    if max_points == 0 {
        return Vec::new();
    }
    let Some(items) = unwrap_series_array(value) else {
        return Vec::new();
    };

    let mut parsed = Vec::with_capacity(items.len());
    for (idx, item) in items.iter().enumerate() {
        let point = match item {
            Value::Array(pair) if pair.len() >= 2 => {
                let ts = parse_timestamp_sec(&pair[0]);
                let val = parse_decimal_string(&pair[1]);
                ts.zip(val)
            }
            Value::Object(map) => {
                let ts = ["timestamp", "ts", "time", "t", "x"]
                    .iter()
                    .find_map(|k| map.get(*k).and_then(parse_timestamp_sec))
                    .unwrap_or(idx as i64);
                let val = ["close", "value", "price", "equity", "v", "y"]
                    .iter()
                    .find_map(|k| map.get(*k).and_then(parse_decimal_string));
                val.map(|v| (ts, v))
            }
            scalar => parse_decimal_string(scalar).map(|v| (idx as i64, v)),
        };
        if let Some(pt) = point {
            parsed.push(pt);
        }
    }

    if parsed.len() <= max_points {
        return parsed;
    }
    if max_points == 1 {
        return vec![parsed.pop().expect("non-empty")];
    }

    let n = parsed.len();
    let mut bounded = Vec::with_capacity(max_points);
    for i in 0..max_points {
        let pick = i * (n - 1) / (max_points - 1);
        bounded.push(parsed[pick].clone());
    }
    bounded
}
