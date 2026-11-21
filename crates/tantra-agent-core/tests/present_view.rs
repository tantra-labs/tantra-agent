use serde_json::{Value, json};
use tantra_agent_core::{DisplayBlock, MetricItem, PresentViewRequest, ViewKind, present_view};

fn req(v: Value) -> Result<PresentViewRequest, serde_json::Error> {
    serde_json::from_value(v)
}

fn ok_req() -> PresentViewRequest {
    req(json!({
        "kind": "strategy_lab", "title": " Lab ", "symbols": ["NIFTY"],
        "summary": "**ok**", "highlights": [{"label":"Sharpe","value":"1.4"}],
        "include_tools": ["strategy_loop_eval"]
    }))
    .unwrap()
}

#[test]
fn valid_request_yields_one_view_block() {
    let env = present_view(&ok_req()).unwrap();
    assert_eq!(env.blocks.len(), 1);
    match &env.blocks[0] {
        DisplayBlock::View {
            kind,
            title,
            symbols,
            summary,
            highlights,
            include_tools,
        } => {
            assert_eq!(*kind, ViewKind::StrategyLab);
            assert_eq!(title, "Lab");
            assert_eq!(symbols, &vec!["NIFTY".to_string()]);
            assert_eq!(summary, "**ok**");
            assert_eq!(highlights.len(), 1);
            assert_eq!(include_tools, &vec!["strategy_loop_eval".to_string()]);
        }
        other => panic!("unexpected block {other:?}"),
    }
    let j = serde_json::to_value(&env.blocks[0]).unwrap();
    assert_eq!(j["type"], "view");
    assert_eq!(j["kind"], "strategy_lab");
}

#[test]
fn unknown_field_and_kind_rejected() {
    assert!(req(json!({"kind":"report","title":"t","extra":1})).is_err());
    assert!(req(json!({"kind":"nope","title":"t"})).is_err());
}

fn bad(f: impl FnOnce(&mut PresentViewRequest)) {
    let mut r = ok_req();
    f(&mut r);
    assert!(present_view(&r).is_err());
}

#[test]
fn caps_and_names_enforced() {
    bad(|r| r.title = "  ".into());
    bad(|r| r.title = "x".repeat(121));
    bad(|r| r.summary = "x".repeat(4001));
    bad(|r| r.symbols = vec!["A".into(); 5]);
    bad(|r| r.symbols = vec![" ".into()]);
    bad(|r| r.symbols = vec!["x".repeat(41)]);
    bad(|r| {
        r.highlights = vec![
            MetricItem {
                label: "a".into(),
                value: "b".into()
            };
            13
        ]
    });
    bad(|r| {
        r.highlights = vec![MetricItem {
            label: "x".repeat(41),
            value: "b".into(),
        }]
    });
    bad(|r| r.include_tools = vec!["a".into(); 17]);
    bad(|r| r.include_tools = vec!["Bad-Name".into()]);
    bad(|r| r.include_tools = vec!["x".repeat(41)]);
}

#[test]
fn block_round_trips() {
    let env = present_view(&ok_req()).unwrap();
    let s = serde_json::to_string(&env.blocks[0]).unwrap();
    let back: DisplayBlock = serde_json::from_str(&s).unwrap();
    assert_eq!(back, env.blocks[0]);
}
