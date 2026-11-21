use serde_json::json;
use tantra_agent_chat::event::{CallId, TurnId};
use tantra_agent_chat::permission::ToolRisk;
use tantra_agent_chat::tool::{ChatTool, ToolContext};
use tantra_agent_chat_tools::{PRESENT_VIEW_GUIDANCE, PresentViewTool, create_presentation_tools};
use tantra_agent_core::ToolError;
use tokio::sync::mpsc;

fn ctx() -> ToolContext {
    let (tx, _) = mpsc::channel(4);
    ToolContext::new(None, TurnId(1), CallId::new("c"), tx)
}

#[test]
fn spec_and_risk() {
    let t = PresentViewTool::new();
    assert_eq!(t.spec().name, "present_view");
    assert_eq!(t.traits().risk, ToolRisk::ReadOnly);
    assert_eq!(create_presentation_tools().len(), 1);
    assert!(PRESENT_VIEW_GUIDANCE.contains("present_view"));
}

#[tokio::test]
async fn execute_returns_view_block() {
    let out = PresentViewTool::new()
        .execute(json!({"kind":"compare","title":"A vs B"}), ctx())
        .await
        .unwrap();
    assert_eq!(out.payload["blocks"][0]["type"], "view");
}

#[tokio::test]
async fn invalid_input_rejected() {
    let r = PresentViewTool::new()
        .execute(json!({"kind":"compare","title":""}), ctx())
        .await;
    assert!(matches!(r, Err(ToolError::InvalidArguments { .. })));
}
