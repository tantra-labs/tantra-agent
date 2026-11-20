//! Indicator library port: a narrow, typed boundary to an external indicator/study library
//! (served by the runtime / MCP) plus dynamic-descriptor loading for the [`IndicatorCatalog`].
//!
//! All inputs from the library are untrusted: descriptors are parsed strictly (no JSON floats)
//! and request DTOs enforce hard caps before crossing the port.
//!
//! [`IndicatorCatalog`]: crate::indicators::IndicatorCatalog

use rust_decimal::Decimal;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::str::FromStr;
use thiserror::Error;

use crate::indicators::ParameterSpec;

/// Maximum bar rows accepted inline.
pub const MAX_BARS: usize = 5000;
/// Maximum tail rows returned.
pub const MAX_TAIL: u32 = 2000;
/// Maximum number of horizons in a signal study.
pub const MAX_HORIZONS: usize = 10;

/// Classification of a library port failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryPortErrorKind {
    /// Library not reachable / not configured.
    Unavailable,
    /// Request or payload failed validation.
    InvalidRequest,
    /// Requested entry does not exist.
    NotFound,
    /// Library is saturated; retry later.
    Busy,
    /// Call exceeded its deadline.
    Timeout,
    /// Upstream returned an error.
    Upstream,
}

/// Typed library port error.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
#[error("library port error ({kind:?}): {message}")]
pub struct LibraryPortError {
    /// Failure class.
    pub kind: LibraryPortErrorKind,
    /// Human-readable message.
    pub message: String,
}

impl LibraryPortError {
    fn make(kind: LibraryPortErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
    /// Library unavailable.
    pub fn unavailable(message: impl Into<String>) -> Self {
        Self::make(LibraryPortErrorKind::Unavailable, message)
    }
    /// Invalid request.
    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::make(LibraryPortErrorKind::InvalidRequest, message)
    }
    /// Entry not found.
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::make(LibraryPortErrorKind::NotFound, message)
    }
    /// Library busy.
    pub fn busy(message: impl Into<String>) -> Self {
        Self::make(LibraryPortErrorKind::Busy, message)
    }
    /// Timeout.
    pub fn timeout(message: impl Into<String>) -> Self {
        Self::make(LibraryPortErrorKind::Timeout, message)
    }
    /// Upstream error.
    pub fn upstream(message: impl Into<String>) -> Self {
        Self::make(LibraryPortErrorKind::Upstream, message)
    }
}

/// Search request.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LibrarySearchRequest {
    /// Free-text query.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    /// Entry kind filter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Family filter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    /// Max results.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

/// Describe output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DescribeFormat {
    /// Markdown text.
    Markdown,
    /// Structured JSON entry.
    Json,
}

/// Describe request.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LibraryDescribeRequest {
    /// Entry id.
    pub id: String,
    /// Output format.
    pub format: DescribeFormat,
}

/// Compute request.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IndicatorComputeRequest {
    /// Entry id.
    pub id: String,
    /// Indicator parameters.
    pub params: Map<String, Value>,
    /// Instrument symbol.
    pub instrument: String,
    /// Bar interval.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interval: Option<String>,
    /// Inline bars (at most 5000).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bars: Option<Vec<Value>>,
    /// Relative bars file path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bars_file: Option<String>,
    /// Tail rows to return (1..=2000).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tail: Option<u32>,
    /// Render overlay scene.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overlay_scene: Option<bool>,
}

/// Signal study request.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SignalStudyRequest {
    /// Entry id.
    pub id: String,
    /// Parameters.
    pub params: Map<String, Value>,
    /// Instrument symbol.
    pub instrument: String,
    /// Bar interval.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interval: Option<String>,
    /// Inline bars (at most 5000).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bars: Option<Vec<Value>>,
    /// Relative bars file path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bars_file: Option<String>,
    /// Event column name.
    pub event_column: String,
    /// Forward horizons (at most 10).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub horizons: Option<Vec<u32>>,
    /// Direction, 1 or -1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<i8>,
    /// Optional filter object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<Map<String, Value>>,
    /// Render overlay scene.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overlay_scene: Option<bool>,
}

fn validate_bars_source(
    bars: &Option<Vec<Value>>,
    bars_file: &Option<String>,
) -> Result<(), LibraryPortError> {
    match (bars, bars_file) {
        (Some(_), Some(_)) => Err(LibraryPortError::invalid_request(
            "provide exactly one of bars or bars_file, not both",
        )),
        (None, None) => Err(LibraryPortError::invalid_request(
            "provide exactly one of bars or bars_file",
        )),
        (Some(b), None) => {
            if b.len() > MAX_BARS {
                Err(LibraryPortError::invalid_request(format!(
                    "bars exceeds {MAX_BARS} rows"
                )))
            } else {
                Ok(())
            }
        }
        (None, Some(f)) => validate_relative_path(f),
    }
}

fn validate_relative_path(path: &str) -> Result<(), LibraryPortError> {
    if path.is_empty()
        || path.starts_with('/')
        || path.contains('\0')
        || path.contains(':')
        || path.contains('\\')
        || path.split('/').any(|seg| seg == "..")
    {
        return Err(LibraryPortError::invalid_request(
            "bars_file must be a relative path without '..', ':', '\\\\' or NUL",
        ));
    }
    Ok(())
}

impl IndicatorComputeRequest {
    /// Enforce size caps and path safety.
    pub fn validate(&self) -> Result<(), LibraryPortError> {
        validate_bars_source(&self.bars, &self.bars_file)?;
        if let Some(t) = self.tail {
            if t == 0 || t > MAX_TAIL {
                return Err(LibraryPortError::invalid_request(format!(
                    "tail must be within 1..={MAX_TAIL}"
                )));
            }
        }
        Ok(())
    }
}

impl SignalStudyRequest {
    /// Enforce size caps and path safety.
    pub fn validate(&self) -> Result<(), LibraryPortError> {
        validate_bars_source(&self.bars, &self.bars_file)?;
        if let Some(h) = &self.horizons {
            if h.len() > MAX_HORIZONS {
                return Err(LibraryPortError::invalid_request(format!(
                    "horizons exceeds {MAX_HORIZONS} entries"
                )));
            }
        }
        if let Some(d) = self.direction {
            if d != 1 && d != -1 {
                return Err(LibraryPortError::invalid_request(
                    "direction must be 1 or -1",
                ));
            }
        }
        Ok(())
    }
}

/// Port to an external indicator library.
#[async_trait::async_trait]
pub trait IndicatorLibraryPort: Send + Sync {
    /// Search entries.
    async fn search(&self, request: LibrarySearchRequest) -> Result<Value, LibraryPortError>;
    /// Describe one entry.
    async fn describe(&self, request: LibraryDescribeRequest) -> Result<Value, LibraryPortError>;
    /// Compute an indicator over bars.
    async fn compute(&self, request: IndicatorComputeRequest) -> Result<Value, LibraryPortError>;
    /// Run a signal study.
    async fn signal_study(&self, request: SignalStudyRequest) -> Result<Value, LibraryPortError>;
}

/// Parsed descriptor of a library entry usable by the catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryIndicatorDescriptor {
    /// Full library id (e.g. `ind.predictive-ranges`).
    pub id: String,
    /// Entry kind.
    pub kind: String,
    /// Title.
    pub title: String,
    /// Aliases.
    pub aliases: Vec<String>,
    /// Numeric (int/decimal) parameters with bounds.
    pub numeric_params: Vec<ParameterSpec>,
    /// Names of enum/bool parameters.
    pub other_params: Vec<String>,
}

fn str_field<'a>(v: &'a Value, key: &str) -> Result<&'a str, LibraryPortError> {
    v.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| LibraryPortError::invalid_request(format!("missing string field '{key}'")))
}

/// Parse an int JSON number or decimal string; reject floats.
fn parse_decimal(v: &Value, what: &str) -> Result<Decimal, LibraryPortError> {
    match v {
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(Decimal::from(i))
            } else if let Some(u) = n.as_u64() {
                Ok(Decimal::from(u))
            } else {
                Err(LibraryPortError::invalid_request(format!(
                    "{what}: JSON float not permitted, use a decimal string"
                )))
            }
        }
        Value::String(s) => Decimal::from_str(s.trim()).map_err(|e| {
            LibraryPortError::invalid_request(format!("{what}: invalid decimal '{s}': {e}"))
        }),
        _ => Err(LibraryPortError::invalid_request(format!(
            "{what}: expected int or decimal string"
        ))),
    }
}

impl LibraryIndicatorDescriptor {
    /// Parse from the library's `Entry.to_dict()` JSON.
    pub fn from_entry_json(v: &Value) -> Result<Self, LibraryPortError> {
        let id = str_field(v, "id")?.to_string();
        let kind = str_field(v, "kind")?.to_string();
        let title = v
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let aliases = v
            .get("aliases")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();

        let mut numeric_params = Vec::new();
        let mut other_params = Vec::new();
        if let Some(params) = v.get("params").and_then(Value::as_array) {
            for p in params {
                let name = str_field(p, "name")?.to_string();
                let ty = str_field(p, "type")?;
                let description = p
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                match ty {
                    "int" | "decimal" => {
                        let bound = |key: &str, fallback: Decimal| match p.get(key) {
                            None | Some(Value::Null) => Ok(fallback),
                            Some(b) => parse_decimal(b, &format!("{id}.{name}.{key}")),
                        };
                        let min = bound("minimum", Decimal::MIN)?;
                        let max = bound("maximum", Decimal::MAX)?;
                        let default = match p.get("default") {
                            None | Some(Value::Null) => Decimal::ZERO,
                            Some(d) => parse_decimal(d, &format!("{id}.{name}.default"))?,
                        };
                        numeric_params.push(ParameterSpec {
                            name,
                            min,
                            max,
                            step: if ty == "int" {
                                Decimal::ONE
                            } else {
                                Decimal::ZERO
                            },
                            default,
                            description,
                        });
                    }
                    "enum" | "bool" => other_params.push(name),
                    other => {
                        return Err(LibraryPortError::invalid_request(format!(
                            "{id}.{name}: unknown param type '{other}'"
                        )));
                    }
                }
            }
        }
        Ok(Self {
            id,
            kind,
            title,
            aliases,
            numeric_params,
            other_params,
        })
    }
}

fn search_items(v: &Value) -> Vec<&Value> {
    let arr = match v {
        Value::Array(a) => Some(a),
        Value::Object(o) => o
            .get("results")
            .or_else(|| o.get("entries"))
            .and_then(Value::as_array),
        _ => None,
    };
    arr.map(|a| a.iter().collect()).unwrap_or_default()
}

/// Load descriptors for computable entries, bounded by `max_entries`.
pub async fn load_library_descriptors(
    port: &dyn IndicatorLibraryPort,
    max_entries: usize,
) -> Result<Vec<LibraryIndicatorDescriptor>, LibraryPortError> {
    let mut out: Vec<LibraryIndicatorDescriptor> = Vec::new();
    if max_entries == 0 {
        return Ok(out);
    }
    // The library server caps `limit` at 200 per search call.
    let limit = u32::try_from(max_entries.min(200)).unwrap_or(200);
    for kind in ["indicator", "composite", "detector", "primitive"] {
        let found = port
            .search(LibrarySearchRequest {
                kind: Some(kind.to_string()),
                limit: Some(limit),
                ..Default::default()
            })
            .await?;
        for item in search_items(&found) {
            if out.len() >= max_entries {
                return Ok(out);
            }
            if item.get("computable").and_then(Value::as_bool) == Some(false) {
                continue;
            }
            let Some(id) = item.get("id").and_then(Value::as_str) else {
                continue;
            };
            if out.iter().any(|d| d.id == id) {
                continue;
            }
            let described = port
                .describe(LibraryDescribeRequest {
                    id: id.to_string(),
                    format: DescribeFormat::Json,
                })
                .await
                .and_then(|entry| LibraryIndicatorDescriptor::from_entry_json(&entry));
            match described {
                Ok(descriptor) => out.push(descriptor),
                // Availability failures abort the load (caller falls back to the static
                // catalog); one malformed/unknown entry is skipped, not fatal.
                Err(e) if is_availability_error(&e) => return Err(e),
                Err(e) => tracing::warn!(id, error = %e, "skipping library entry"),
            }
        }
        if out.len() >= max_entries {
            break;
        }
    }
    Ok(out)
}

fn is_availability_error(e: &LibraryPortError) -> bool {
    matches!(
        e.kind,
        LibraryPortErrorKind::Unavailable
            | LibraryPortErrorKind::Timeout
            | LibraryPortErrorKind::Busy
    )
}
