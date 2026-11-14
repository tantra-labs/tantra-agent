//! Tests for the universal `_jq` JSONPath response filter
//! (plan 0007 Track 6 Phase 15).

use serde_json::json;
use tantra_agent_core::jq::{JqError, apply_jq_filter};

#[test]
fn filters_nested_object() {
    let payload = json!({"a": {"b": [1, 2, 3]}, "c": true});
    assert_eq!(apply_jq_filter(&payload, "$.a.b[1]").unwrap(), json!(2));
}

#[test]
fn filters_array_elements() {
    let payload = json!({"tools": [{"name": "x"}, {"name": "y"}]});
    assert_eq!(
        apply_jq_filter(&payload, "$.tools[*].name").unwrap(),
        json!("x")
    );
}

#[test]
fn invalid_expression_errors() {
    assert_eq!(
        apply_jq_filter(&json!({}), "$["),
        Err(JqError::InvalidExpression("$[".to_string()))
    );
}

#[test]
fn no_match_errors() {
    assert_eq!(apply_jq_filter(&json!({}), "$.nope"), Err(JqError::NoMatch));
}
