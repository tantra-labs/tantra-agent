//! Contract test: the routes and strategy-loop request fields `RuntimeLibraryClient` uses must
//! exist (and not be deprecated) in the runtime's committed `openapi.v2.json`.

use serde_json::Value;
use tantra_agent_mcp::RUNTIME_ROUTES;

const ARTIFACT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../tantra-runtime/crates/tantra-runtime-api-rest/schema/openapi.v2.json"
);

fn load() -> Option<Value> {
    let text = match std::fs::read_to_string(ARTIFACT) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("skipping runtime contract test: {ARTIFACT}: {e}");
            return None;
        }
    };
    Some(serde_json::from_str(&text).expect("openapi.v2.json is valid JSON"))
}

fn request_schema<'a>(doc: &'a Value, path: &str) -> &'a Value {
    &doc["paths"][path]["post"]["requestBody"]["content"]["application/json"]["schema"]
}

/// Resolve `{"$ref"}` / `{"allOf":[{"$ref"}]}` against `components.schemas` / `definitions`.
fn resolve<'a>(doc: &'a Value, schema: &'a Value) -> &'a Value {
    let r = schema
        .get("$ref")
        .or_else(|| schema.pointer("/allOf/0/$ref"))
        .and_then(Value::as_str);
    match r.and_then(|r| r.rsplit('/').next()) {
        Some(name) => doc
            .pointer(&format!("/components/schemas/{name}"))
            .or_else(|| doc.pointer(&format!("/definitions/{name}")))
            .unwrap_or(schema),
        None => schema,
    }
}

fn assert_fields(doc: &Value, schema: &Value, fields: &[&str], what: &str) {
    let props = resolve(doc, schema)
        .get("properties")
        .and_then(Value::as_object)
        .unwrap_or_else(|| panic!("{what}: schema has no resolvable properties"));
    for f in fields {
        assert!(props.contains_key(*f), "{what}: field `{f}` not in schema");
    }
}

#[test]
fn routes_exist_and_are_not_deprecated() {
    let Some(doc) = load() else { return };
    for (method, path) in RUNTIME_ROUTES {
        let op = &doc["paths"][*path][method.to_lowercase()];
        assert!(
            op.is_object(),
            "{method} {path} missing from openapi.v2.json"
        );
        assert_ne!(
            op.get("deprecated").and_then(Value::as_bool),
            Some(true),
            "{method} {path} is deprecated in openapi.v2.json"
        );
    }
}

#[test]
fn strategy_loop_request_fields_are_accepted() {
    let Some(doc) = load() else { return };
    let eval_fields = [
        "strategy_id",
        "instrument_id",
        "asset_class",
        "start_date",
        "end_date",
        "timeframe",
        "session_preset",
        "entry_trigger_mode",
        "stop_policy_mode",
        "sl_buffer_atr",
        "horizon_bars",
        "target_r",
        "atr_period",
        "partial_r",
        "runner_r",
        "time_stop_bars",
        "expiry_horizon",
        "lot_size",
        "tick_size",
        "capital",
        "session_id",
        "max_chart_bars",
    ];
    let base = "/studies/strategy-loop/evaluate";
    assert_fields(&doc, request_schema(&doc, base), &eval_fields, "evaluate");

    let sweep = request_schema(&doc, "/studies/strategy-loop/sweep");
    assert_fields(&doc, sweep, &["base", "bounds"], "sweep");
    let bounds = &resolve(&doc, sweep)["properties"]["bounds"];
    assert_fields(
        &doc,
        bounds,
        &[
            "sl_buffer_atr_grid",
            "horizon_bars_grid",
            "target_r_grid",
            "max_candidates",
        ],
        "sweep.bounds",
    );

    let hedge = request_schema(&doc, "/studies/strategy-loop/recommend-hedge");
    assert_fields(
        &doc,
        hedge,
        &[
            "instrument_id",
            "asset_class",
            "strategy_id",
            "start_date",
            "end_date",
            "timeframe",
            "direction",
            "expiry_horizon",
            "sl_buffer_atr",
            "horizon_bars",
            "atr_period",
            "lot_size",
            "tick_size",
            "capital",
        ],
        "recommend-hedge",
    );
}
