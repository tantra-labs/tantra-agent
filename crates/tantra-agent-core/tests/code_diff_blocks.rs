//! Serialization and schema tests for `DisplayBlock::CodeDiff` (`tantra/result/v1`).

use serde_json::json;
use tantra_agent_core::{CodeDiffHunk, DisplayBlock, TantraResultEnvelope};

#[test]
fn code_diff_display_block_round_trips_and_defaults_cleanly() {
    let block = DisplayBlock::CodeDiff {
        title: "EMA Crossover v2".to_string(),
        lang: "python".to_string(),
        code: "def on_bar(bar):\n    return True\n".to_string(),
        hunks: vec![CodeDiffHunk {
            id: "h1".to_string(),
            header: "@@ -1,1 +1,2 @@".to_string(),
            lines: vec![
                " def on_bar(bar):".to_string(),
                "+    return True".to_string(),
            ],
        }],
    };

    let env =
        TantraResultEnvelope::ok("Proposed EMA tweak", json!({ "ok": true })).with_block(block);
    let encoded = serde_json::to_value(&env).expect("serialize envelope");
    assert_eq!(encoded["blocks"][0]["type"], "code_diff");
    assert_eq!(encoded["blocks"][0]["lang"], "python");
    assert_eq!(encoded["blocks"][0]["hunks"][0]["id"], "h1");

    let decoded: TantraResultEnvelope<serde_json::Value> =
        serde_json::from_value(encoded).expect("deserialize envelope");
    assert_eq!(decoded.blocks.len(), 1);
    assert!(matches!(
        &decoded.blocks[0],
        DisplayBlock::CodeDiff { title, lang, hunks, .. }
            if title == "EMA Crossover v2" && lang == "python" && hunks.len() == 1
    ));

    // Omitted optional `code` and `hunks` default cleanly.
    let minimal: DisplayBlock = serde_json::from_value(json!({
        "type": "code_diff",
        "title": "Minimal Pine",
        "lang": "pine"
    }))
    .expect("deserialize minimal code_diff");
    assert!(matches!(
        minimal,
        DisplayBlock::CodeDiff { ref code, ref hunks, .. } if code.is_empty() && hunks.is_empty()
    ));
}
