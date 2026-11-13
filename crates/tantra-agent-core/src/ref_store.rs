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
use serde::{Deserialize, Serialize};
use serde_json::Value;

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

impl FromIterator<(String, Value)> for DataReferenceStore {
    fn from_iter<T: IntoIterator<Item = (String, Value)>>(iter: T) -> Self {
        let store = DataReferenceStore::new();
        for (id, data) in iter {
            store.export(id, data);
        }
        store
    }
}
