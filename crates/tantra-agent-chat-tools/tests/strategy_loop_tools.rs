use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{Value, json};
use tantra_agent_chat::event::{CallId, TurnId};
use tantra_agent_chat::permission::ToolRisk;
use tantra_agent_chat::tool::{ChatTool, ToolContext};
use tantra_agent_chat_tools::create_strategy_loop_tools;
use tantra_agent_core::{
    DisplayBlock, StrategyHedgeRequest, StrategyLoopEvalRequest, StrategyLoopPort,
    StrategyLoopPortError, StrategyLoopSweepRequest, ToolError,
};
use tokio::sync::mpsc;

struct Fake {
    fail: bool,
}

fn canned() -> Value {
    json!({
        "kpis": {"win_rate": "0.55", "trades": 12, "nested": {"x": 1}},
        "scene": {"signals": [], "bands": [], "stops": [], "indicators": [], "hints": [], "hedges": []},
        "bars": [{"time": "2026-01-01T00:00:00Z"}, {"time": "2026-01-02T00:00:00Z"}],
        "recommended": {"strike": "22000", "side": "put"}
    })
}

#[async_trait]
impl StrategyLoopPort for Fake {
    async fn evaluate(&self, _r: StrategyLoopEvalRequest) -> Result<Value, StrategyLoopPortError> {
        if self.fail {
            return Err(StrategyLoopPortError::unavailable("down"));
        }
        Ok(canned())
    }
    async fn sweep(&self, _r: StrategyLoopSweepRequest) -> Result<Value, StrategyLoopPortError> {
        let c = canned();
        Ok(json!({
            "winner": {"oos_kpis": {"win_rate_pct": "61.5"}},
            "winner_scene": c["scene"],
            "baseline_scene": c["scene"],
            "bars": c["bars"],
        }))
    }
    async fn recommend_hedge(
        &self,
        _r: StrategyHedgeRequest,
    ) -> Result<Value, StrategyLoopPortError> {
        Ok(canned())
    }
}

fn ctx() -> ToolContext {
    let (tx, _) = mpsc::channel(4);
    ToolContext::new(None, TurnId(1), CallId::new("c"), tx)
}

fn tools(fail: bool) -> Vec<Arc<dyn ChatTool>> {
    create_strategy_loop_tools(Arc::new(Fake { fail }))
}

fn find(ts: &[Arc<dyn ChatTool>], name: &str) -> Arc<dyn ChatTool> {
    ts.iter().find(|t| t.spec().name == name).cloned().unwrap()
}

fn eval_args() -> Value {
    json!({"strategy_id":"s","instrument_id":"NIFTY","asset_class":"index",
           "start_date":"2026-01-01","end_date":"2026-02-01","timeframe":"15m"})
}

#[test]
fn names_and_risk() {
    let ts = tools(false);
    let names: Vec<_> = ts.iter().map(|t| t.spec().name.clone()).collect();
    assert_eq!(
        names,
        [
            "strategy_loop_eval",
            "strategy_loop_sweep",
            "strategy_hedge_recommend",
            "chart_scene_annotate"
        ]
    );
    assert!(ts.iter().all(|t| t.traits().risk == ToolRisk::ReadOnly));
}

#[tokio::test]
async fn eval_builds_grid_and_overlay_with_axis() {
    let out = find(&tools(false), "strategy_loop_eval")
        .execute(eval_args(), ctx())
        .await
        .unwrap();
    assert!(out.ok);
    let blocks = out.payload["blocks"].as_array().unwrap();
    assert_eq!(blocks[0]["type"], "metric_grid");
    let items = blocks[0]["items"].as_array().unwrap();
    assert_eq!(items.len(), 2); // nested skipped
    assert_eq!(blocks[1]["type"], "overlay_scene");
    assert_eq!(blocks[1]["title"], "s on NIFTY");
    assert_eq!(blocks[1]["timeframe"], "15m");
    assert_eq!(
        blocks[1]["time_axis"],
        json!([1767225600i64, 1767312000i64])
    );
    let _: DisplayBlock = serde_json::from_value(blocks[1].clone()).unwrap();
}

#[tokio::test]
async fn hedge_grid_from_recommended() {
    let out = find(&tools(false), "strategy_hedge_recommend")
        .execute(
            json!({"instrument_id":"NIFTY","asset_class":"index"}),
            ctx(),
        )
        .await
        .unwrap();
    assert_eq!(
        out.payload["blocks"][0]["items"].as_array().unwrap().len(),
        2
    );
}

#[tokio::test]
async fn port_error_and_bad_args() {
    let e = find(&tools(true), "strategy_loop_eval")
        .execute(eval_args(), ctx())
        .await
        .err()
        .unwrap();
    assert!(matches!(e, ToolError::Execution(..)));
    let e = find(&tools(false), "strategy_loop_eval")
        .execute(json!({"strategy_id":"s","bogus":1}), ctx())
        .await
        .err()
        .unwrap();
    assert!(matches!(e, ToolError::InvalidArguments { .. }));
    let mut bad = eval_args();
    bad["start_date"] = json!("nope");
    let e = find(&tools(false), "strategy_loop_eval")
        .execute(bad, ctx())
        .await
        .err()
        .unwrap();
    assert!(matches!(e, ToolError::InvalidArguments { .. }));
}

#[tokio::test]
async fn scene_annotate_is_pure() {
    let out = find(&tools(true), "chart_scene_annotate")
        .execute(
            json!({"title":"T","symbol":"X","timeframe":"1d","scene":canned()["scene"]}),
            ctx(),
        )
        .await
        .unwrap();
    assert_eq!(out.payload["blocks"][1]["type"], "overlay_scene");
}

#[tokio::test]
async fn eval_payload_drops_bars_and_duplicated_scene() {
    let out = find(&tools(false), "strategy_loop_eval")
        .execute(eval_args(), ctx())
        .await
        .unwrap();
    let data = &out.payload["data"];
    assert!(data.get("bars").is_none() && data.get("scene").is_none());
    assert!(data.get("kpis").is_some());
}

#[tokio::test]
async fn sweep_uses_winner_scene_and_kpis() {
    let args = json!({"base": eval_args(), "bounds": {"sl_buffer_atr_grid": ["0.25", "0.5"]}});
    let out = find(&tools(false), "strategy_loop_sweep")
        .execute(args, ctx())
        .await
        .unwrap();
    let blocks = out.payload["blocks"].as_array().unwrap();
    assert_eq!(blocks[0]["type"], "metric_grid");
    assert_eq!(blocks[1]["type"], "overlay_scene");
    assert_eq!(
        blocks[1]["time_axis"],
        json!([1767225600i64, 1767312000i64])
    );
    let data = &out.payload["data"];
    assert!(data.get("winner_scene").is_none() && data.get("baseline_scene").is_none());
    assert!(data.get("bars").is_none());
}
