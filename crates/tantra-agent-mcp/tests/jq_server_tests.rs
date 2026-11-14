//! Server-level tests for the universal `_jq` response filter
//! (plan 0007 Track 6 Phase 15).

use serde_json::json;
use tantra_agent_mcp::{McpServer, RpcMessage};

#[tokio::test]
async fn jq_filter_extracts_single_field() {
    let server = McpServer::new();
    let msg = RpcMessage::Request {
        id: json!(1),
        method: "tools/call".into(),
        params: json!({
            "name": "tantra_monte_carlo",
            "arguments": {
                "returns": [0.01, -0.005, 0.02, -0.003, 0.015],
                "num_sims": 10,
                "horizon": 5,
                "_jq": "$.data.simulations_run"
            }
        }),
    };
    let resp = server.handle_message(&msg).await.expect("response");
    let RpcMessage::Response { result, error, .. } = resp else {
        panic!("expected Response, got {resp:?}");
    };
    assert!(error.is_none(), "unexpected error: {error:?}");
    let result = result.expect("result present");
    assert_eq!(result["structuredContent"]["data"], json!(10));
}

#[tokio::test]
async fn jq_absent_returns_full_envelope() {
    let server = McpServer::new();
    let msg = RpcMessage::Request {
        id: json!(1),
        method: "tools/call".into(),
        params: json!({
            "name": "tantra_monte_carlo",
            "arguments": {
                "returns": [0.01, -0.005, 0.02, -0.003, 0.015],
                "num_sims": 10,
                "horizon": 5
            }
        }),
    };
    let resp = server.handle_message(&msg).await.expect("response");
    let RpcMessage::Response { result, error, .. } = resp else {
        panic!("expected Response, got {resp:?}");
    };
    assert!(error.is_none(), "unexpected error: {error:?}");
    let result = result.expect("result present");
    assert!(
        result["structuredContent"]["data"].is_object(),
        "data should be a full object without _jq"
    );
}
