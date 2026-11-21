use serde_json::json;
use tantra_agent_mcp::{McpError, McpServer, list_tool_definitions};

#[test]
fn definitions_present_and_closed() {
    let defs = list_tool_definitions();
    for name in [
        "strategy_loop_eval",
        "strategy_loop_sweep",
        "strategy_hedge_recommend",
        "chart_scene_annotate",
    ] {
        let d = defs.iter().find(|d| d["name"] == name).unwrap();
        assert_eq!(
            d["inputSchema"]["additionalProperties"],
            json!(false),
            "{name}"
        );
    }
    let sweep = defs
        .iter()
        .find(|d| d["name"] == "strategy_loop_sweep")
        .unwrap();
    assert_eq!(
        sweep["inputSchema"]["properties"]["base"]["additionalProperties"],
        json!(false)
    );
}

#[tokio::test]
async fn loop_tool_without_port_is_unavailable() {
    let s = McpServer::new();
    let r = s
        .call_tool(
            "strategy_loop_eval",
            json!({"strategy_id":"s","instrument_id":"N","asset_class":"index",
                   "start_date":"2026-01-01","end_date":"2026-02-01"}),
        )
        .await;
    assert!(matches!(r, Err(McpError::Unavailable(_))));
}

#[tokio::test]
async fn scene_annotate_works_without_port() {
    let s = McpServer::new();
    let r = s
        .call_tool(
            "chart_scene_annotate",
            json!({"title":"T","symbol":"X","timeframe":"1d",
                   "scene":{"signals":[],"bands":[],"stops":[],"indicators":[],"hints":[],"hedges":[]}}),
        )
        .await
        .unwrap();
    assert_eq!(r["blocks"][1]["type"], "overlay_scene");
    let bad = s
        .call_tool(
            "chart_scene_annotate",
            json!({"title":"T","symbol":"X","timeframe":"1d","scene":{}}),
        )
        .await;
    assert!(matches!(bad, Err(McpError::InvalidArguments(_))));
}
