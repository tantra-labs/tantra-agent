//! Regression: every `SessionEvent` the WS streams must serialize. `ToolDisplay::Text` used to be
//! an internally tagged newtype over `String`, which serde rejects, so successful
//! `tool_finished` events were dropped by the runtime's chat socket.

use tantra_agent_chat::event::{ToolDisplay, ToolOutcome};

#[test]
fn tool_display_text_serializes_adjacently_tagged() {
    let json = serde_json::to_value(ToolDisplay::Text("{\"ok\":true}".into())).expect("serialize");
    assert_eq!(json["kind"], "text");
    assert_eq!(json["value"], "{\"ok\":true}");
    let back: ToolDisplay = serde_json::from_value(json).expect("round-trip");
    assert_eq!(back, ToolDisplay::Text("{\"ok\":true}".into()));
}

#[test]
fn tool_display_table_serializes() {
    let t = ToolDisplay::Table {
        columns: vec!["a".into()],
        rows: vec![vec!["1".into()]],
    };
    let json = serde_json::to_value(&t).expect("serialize");
    assert_eq!(json["kind"], "table");
    assert_eq!(json["value"]["columns"][0], "a");
}

#[test]
fn tool_outcome_serializes() {
    assert!(serde_json::to_string(&ToolOutcome::Success).is_ok());
    assert!(serde_json::to_string(&ToolOutcome::Error("x".into())).is_ok());
}
