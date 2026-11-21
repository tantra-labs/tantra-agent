use rust_decimal_macros::dec;
use serde_json::json;
use tantra_agent_core::strategy_loop::StrategyLoopPortErrorKind;
use tantra_agent_core::{
    DisplayBlock, SceneAnnotateRequest, StrategyHedgeRequest, StrategyLoopBounds,
    StrategyLoopEvalRequest, StrategyLoopSweepRequest, scene_annotation,
};

fn eval() -> StrategyLoopEvalRequest {
    serde_json::from_value(json!({
        "strategy_id": "pr_trend", "instrument_id": "NSE:NIFTY50-INDEX",
        "asset_class": "index", "start_date": "2026-01-01", "end_date": "2026-06-30"
    }))
    .unwrap()
}

fn kind<T>(r: Result<T, tantra_agent_core::StrategyLoopPortError>) -> StrategyLoopPortErrorKind {
    r.err().unwrap().kind
}

#[test]
fn deny_unknown_fields() {
    let r: Result<StrategyLoopEvalRequest, _> = serde_json::from_value(json!({
        "strategy_id": "a", "instrument_id": "b", "asset_class": "index",
        "start_date": "2026-01-01", "end_date": "2026-01-02", "bogus": 1
    }));
    assert!(r.is_err());
    let r: Result<StrategyHedgeRequest, _> = serde_json::from_value(json!({
        "instrument_id": "b", "asset_class": "index", "nope": 1
    }));
    assert!(r.is_err());
}

#[test]
fn valid_eval_passes_and_omits_none() {
    let e = eval();
    e.validate().unwrap();
    let v = serde_json::to_value(&e).unwrap();
    assert!(v.get("timeframe").is_none());
}

#[test]
fn eval_validate_negatives() {
    let mut e = eval();
    e.start_date = "2026-13-01".into();
    assert_eq!(
        kind(e.validate()),
        StrategyLoopPortErrorKind::InvalidRequest
    );
    let mut e = eval();
    e.instrument_id = "  ".into();
    assert!(e.validate().is_err());
    let mut e = eval();
    e.sl_buffer_atr = Some(dec!(0));
    assert!(e.validate().is_err());
    let mut e = eval();
    e.start_date = "2026-07-01".into();
    assert!(e.validate().is_err());
}

#[test]
fn sweep_candidate_caps() {
    let big = StrategyLoopBounds {
        sl_buffer_atr_grid: Some((1..=5).map(rust_decimal::Decimal::from).collect()),
        horizon_bars_grid: Some((1..=5).collect()),
        target_r_grid: Some((1..=5).map(rust_decimal::Decimal::from).collect()),
        max_candidates: None,
    };
    let req = StrategyLoopSweepRequest {
        base: eval(),
        bounds: big,
    };
    assert!(req.validate().is_err()); // 125 > 64
    let long = StrategyLoopBounds {
        horizon_bars_grid: Some((1..=17).collect()),
        ..Default::default()
    };
    assert!(long.validate().is_err());
    let bad_max = StrategyLoopBounds {
        max_candidates: Some(65),
        ..Default::default()
    };
    assert!(bad_max.validate().is_err());
    let ok = StrategyLoopBounds {
        horizon_bars_grid: Some(vec![5, 10]),
        target_r_grid: Some(vec![dec!(1.5), dec!(2)]),
        ..Default::default()
    };
    ok.validate().unwrap();
}

fn scene_req(scene: serde_json::Value) -> SceneAnnotateRequest {
    serde_json::from_value(json!({
        "title": "T", "symbol": "NIFTY", "timeframe": "15m", "scene": scene,
        "time_axis": [1, 2], "kpis": {"win_rate": "0.5"}, "notes": "hello"
    }))
    .unwrap()
}

#[test]
fn scene_annotation_counts_and_overlay_block() {
    let scene =
        json!({"signals":[1,2],"bands":[],"stops":[1],"indicators":[],"hints":[],"hedges":[1]});
    let env = scene_annotation(&scene_req(scene)).unwrap();
    assert!(env.summary.contains("2 signals"));
    assert!(env.summary.contains("1 hedges"));
    assert_eq!(env.data["counts"]["stops"], 1);
    assert!(matches!(env.blocks[0], DisplayBlock::MetricGrid { .. }));
    assert!(matches!(env.blocks[1], DisplayBlock::OverlayScene { .. }));
    assert!(matches!(env.blocks[2], DisplayBlock::Alert { .. }));
    let v = serde_json::to_value(&env.blocks[1]).unwrap();
    assert_eq!(v["type"], "overlay_scene");
    assert_eq!(v["time_axis"], json!([1, 2]));
}

#[test]
fn scene_missing_arrays_rejected() {
    let r = scene_annotation(&scene_req(json!({"signals":[]})));
    assert_eq!(kind(r), StrategyLoopPortErrorKind::InvalidRequest);
    let r = scene_annotation(&scene_req(json!([1])));
    assert!(r.is_err());
}
