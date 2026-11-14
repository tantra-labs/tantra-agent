//! Tests for the Track 4 advanced engines MCP tools: regimes, change-point,
//! copula tail risk, permutation tests, and multi-period optimization.

use serde_json::json;
use tantra_agent_mcp::server::McpServer;
use tantra_agent_mcp::{AllocationToolService, McpError, RegimeToolService};

/// 50 positive-mean then 50 negative-mean returns.
fn two_block_returns() -> Vec<f64> {
    let mut returns = Vec::with_capacity(100);
    for i in 0..50 {
        returns.push(0.004 + 0.002 * ((i as f64) * 0.37).sin());
    }
    for i in 0..50 {
        returns.push(-0.004 + 0.002 * ((i as f64) * 0.37).sin());
    }
    returns
}

/// 100 points of ~1.0 then 100 points of ~5.0 with tiny deterministic jitter.
fn shifted_series() -> Vec<f64> {
    let mut series = Vec::with_capacity(200);
    for i in 0..200 {
        let base = if i < 100 { 1.0 } else { 5.0 };
        series.push(base + (i as f64) * 1e-6);
    }
    series
}

#[test]
fn detect_regimes_envelope() {
    let value = RegimeToolService::new()
        .detect_regimes(&json!({
            "returns": two_block_returns(),
            "n_states": 2
        }))
        .expect("detect_regimes succeeds");
    assert_eq!(value["schema"], "tantra/result/v1");
    assert_eq!(value["status"], "ok");
    let states = value["data"]["states"].as_array().expect("states array");
    assert_eq!(states.len(), 100);
    assert_eq!(value["data"]["n_states"], 2);
    let blocks = value["blocks"].as_array().expect("blocks array");
    assert!(blocks.iter().any(|b| b["type"] == "metric_grid"));
}

#[test]
fn change_point_envelope() {
    let value = RegimeToolService::new()
        .change_point(&json!({ "series": shifted_series() }))
        .expect("change_point succeeds");
    assert_eq!(value["schema"], "tantra/result/v1");
    assert_eq!(value["status"], "ok");
    assert_eq!(value["data"]["detected"], true);
    let break_points = value["data"]["break_points"]
        .as_array()
        .expect("break_points array");
    assert!(!break_points.is_empty());
}

#[test]
fn copula_envelope() {
    let x: Vec<f64> = (0..200).map(|i| (i as f64 * 0.31).sin()).collect();
    let y: Vec<f64> = x
        .iter()
        .enumerate()
        .map(|(i, &v)| v + 1e-6 * (i as f64).sin())
        .collect();
    let value = RegimeToolService::new()
        .copula_tail_risk(&json!({
            "series_x": x,
            "series_y": y,
            "family": "clayton"
        }))
        .expect("copula_tail_risk succeeds");
    assert_eq!(value["schema"], "tantra/result/v1");
    assert_eq!(value["status"], "ok");
    assert_eq!(value["data"]["family"], "clayton");
    assert!(value["data"]["theta"].as_f64().unwrap() > 0.0);
}

#[test]
fn permutation_envelope() {
    let value = RegimeToolService::new()
        .permutation_test(&json!({
            "returns": two_block_returns(),
            "n_permutations": 200
        }))
        .expect("permutation_test succeeds");
    assert_eq!(value["schema"], "tantra/result/v1");
    assert_eq!(value["status"], "ok");
    let p = value["data"]["p_value"].as_f64().unwrap();
    assert!((0.0..=1.0).contains(&p));
    assert!(value["data"]["observed_stat"].as_f64().unwrap().is_finite());
}

#[test]
fn multi_period_optimize_envelope() {
    let value = AllocationToolService::new()
        .multi_period_optimize(&json!({
            "covariance_matrix": [[0.04, 0.01], [0.01, 0.16]],
            "expected_returns": [0.08, 0.06],
            "objective": "mean_variance",
            "periods": 3,
            "symbols": ["A", "B"]
        }))
        .expect("multi_period_optimize succeeds");
    assert_eq!(value["schema"], "tantra/result/v1");
    assert_eq!(value["status"], "ok");
    let data = &value["data"];
    let steps = data["steps"].as_array().expect("steps array");
    assert_eq!(steps.len(), 3);
    let total_cost: f64 = data["total_transaction_cost"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!(total_cost >= 0.0);
    for step in steps {
        let weights = step["target_weights"]
            .as_array()
            .expect("target_weights array");
        let sum: f64 = weights
            .iter()
            .map(|w| w["weight"].as_str().unwrap().parse::<f64>().unwrap())
            .sum();
        assert!((sum - 1.0).abs() < 1e-6, "step weights sum to {sum}");
    }
}

#[tokio::test]
async fn server_call_tool_dispatch() {
    let server = McpServer::new();
    let value = server
        .call_tool(
            "tantra_detect_regimes",
            json!({
                "returns": [0.01, 0.02, -0.01, 0.005, -0.005, 0.015],
                "n_states": 2
            }),
        )
        .await
        .expect("tool call succeeds");
    assert_eq!(value["schema"], "tantra/result/v1");
    assert_eq!(value["status"], "ok");
    let states = value["data"]["states"].as_array().expect("states array");
    assert_eq!(states.len(), 6);
}

#[test]
fn invalid_args_errors() {
    let err = RegimeToolService::new()
        .detect_regimes(&json!({
            "returns": [0.01, 0.02, -0.01, 0.005],
            "n_states": 4
        }))
        .expect_err("n_states = 4 must be rejected");
    assert!(matches!(err, McpError::InvalidArguments(_)));
}
