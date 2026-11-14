//! Universal `_jq` response filter (plan 0007 Track 6 Phase 15): applies a
//! JSONPath expression to a tool payload before serialization, so clients can
//! slash token usage on large envelopes.

use thiserror::Error;

/// Errors raised while applying a `_jq` JSONPath filter to a payload.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum JqError {
    /// The JSONPath expression failed to parse.
    #[error("invalid _jq expression: {0}")]
    InvalidExpression(String),
    /// The JSONPath expression matched nothing in the payload.
    #[error("_jq expression matched nothing in the payload")]
    NoMatch,
}

/// Apply a JSONPath `_jq` filter to `payload` and return the first matched
/// node, cloned into an owned [`serde_json::Value`].
///
/// The dialect is RFC 9535 JSONPath (as implemented by `serde_json_path`),
/// e.g. `$.data.simulations_run` or `$.tools[*].name`.
pub fn apply_jq_filter(
    payload: &serde_json::Value,
    filter: &str,
) -> Result<serde_json::Value, JqError> {
    let path = serde_json_path::JsonPath::parse(filter)
        .map_err(|_| JqError::InvalidExpression(filter.to_string()))?;
    let node = path.query(payload).first().ok_or(JqError::NoMatch)?;
    Ok(node.clone())
}
