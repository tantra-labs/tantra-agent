use serde_json::json;
use tantra_agent_mcp::{McpError, McpServer, list_tool_definitions};

#[test]
fn definition_is_closed() {
    let defs = list_tool_definitions();
    let d = defs.iter().find(|d| d["name"] == "present_view").unwrap();
    assert_eq!(d["inputSchema"]["additionalProperties"], json!(false));
    assert_eq!(d["inputSchema"]["required"], json!(["kind", "title"]));
}

#[tokio::test]
async fn present_view_returns_view_block() {
    let v = McpServer::new()
        .call_tool("present_view", json!({"kind":"options","title":"T"}))
        .await
        .unwrap();
    assert_eq!(v["blocks"][0]["type"], "view");
}

#[tokio::test]
async fn bad_input_is_invalid_arguments() {
    let s = McpServer::new();
    let r = s
        .call_tool("present_view", json!({"kind":"options","title":""}))
        .await;
    assert!(matches!(r, Err(McpError::InvalidArguments(_))));
    let r = s
        .call_tool("present_view", json!({"kind":"zzz","title":"t"}))
        .await;
    assert!(matches!(r, Err(McpError::InvalidArguments(_))));
}
