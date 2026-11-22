//! `ToolFinished` carries the tool's `tantra/result/v1` presentation blocks so UIs
//! (the TUI analysis page, web clients) can render tables, metric grids and charts.

use serde_json::json;
use tantra_agent_chat::event::display_blocks_from_payload;
use tantra_agent_chat::{CallId, SessionEvent, ToolOutcome, TurnId};
use tantra_agent_core::{CodeDiffHunk, DisplayBlock, MetricItem, TantraResultEnvelope};

#[test]
fn blocks_are_extracted_from_a_result_envelope_payload() {
    let env =
        TantraResultEnvelope::ok("backtest: NIFTY", json!({ "sharpe": "1.44" })).with_blocks(vec![
            DisplayBlock::MetricGrid {
                items: vec![MetricItem {
                    label: "Sharpe".into(),
                    value: "1.44".into(),
                }],
            },
            DisplayBlock::Table {
                title: "Trades".into(),
                columns: vec!["Side".into()],
                rows: vec![vec!["BUY".into()]],
            },
            DisplayBlock::CodeDiff {
                title: "EMA Entry Diff".into(),
                lang: "python".into(),
                code: "if ema_fast > ema_slow:\n    buy()".into(),
                hunks: vec![CodeDiffHunk {
                    id: "h1".into(),
                    header: "@@ -1,1 +1,2 @@".into(),
                    lines: vec!["+    buy()".into()],
                }],
            },
        ]);
    let payload = serde_json::to_value(env).unwrap();
    let blocks = display_blocks_from_payload(&payload);
    assert_eq!(blocks.len(), 3);
    assert!(matches!(blocks[0], DisplayBlock::MetricGrid { .. }));
    assert!(matches!(blocks[1], DisplayBlock::Table { ref title, .. } if title == "Trades"));
    assert!(matches!(blocks[2], DisplayBlock::CodeDiff { ref lang, .. } if lang == "python"));
}

#[test]
fn non_envelope_or_malformed_payloads_yield_no_blocks() {
    assert!(display_blocks_from_payload(&json!({ "price": "24570" })).is_empty());
    assert!(display_blocks_from_payload(&json!("plain text")).is_empty());
    // one malformed block is skipped; the well-formed one survives
    let mixed = json!({ "blocks": [
        { "type": "alert", "level": "warning", "message": "Price band" },
        { "type": "no_such_block" }
    ]});
    let blocks = display_blocks_from_payload(&mixed);
    assert_eq!(blocks.len(), 1);
}

#[test]
fn tool_finished_serializes_blocks_and_raw_output_only_when_present() {
    let ev = SessionEvent::ToolFinished {
        turn: TurnId(1),
        call_id: CallId::new("c1".to_string()),
        name: "backtest".into(),
        outcome: ToolOutcome::Success,
        elapsed_ms: 5,
        display: None,
        blocks: vec![],
        raw_output: None,
    };
    let v = serde_json::to_value(&ev).unwrap();
    assert!(v.get("blocks").is_none());
    assert!(v.get("raw_output").is_none());

    let ev = SessionEvent::ToolFinished {
        turn: TurnId(1),
        call_id: CallId::new("c1".to_string()),
        name: "backtest".into(),
        outcome: ToolOutcome::Success,
        elapsed_ms: 5,
        display: None,
        blocks: vec![DisplayBlock::Alert {
            level: "info".into(),
            message: "ok".into(),
        }],
        raw_output: Some(json!({ "sharpe": "1.44" })),
    };
    let v = serde_json::to_value(&ev).unwrap();
    assert_eq!(v["blocks"][0]["type"], "alert");
    assert_eq!(v["raw_output"]["sharpe"], "1.44");
}
