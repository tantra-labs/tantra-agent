//! Integration tests for the MCP stdio client bridge (plan 0007 Track 6
//! Phase 4).
//!
//! These spawn the real `mcp_stdio_server` binary over a tokio stdio pipe and
//! exercise the JSON-RPC 2.0 handshake, tool listing, tool calls, error
//! surfacing, and shutdown. The client is dropped at test exit, which kills
//! the child process even on panic.

use serde_json::json;
use tantra_agent_mcp::{McpClient, McpClientError};

fn spawn_client() -> McpClient {
    McpClient::spawn(env!("CARGO_BIN_EXE_mcp_stdio_server"), &[]).expect("spawn MCP server")
}

#[tokio::test]
async fn spawn_connects_and_initializes() {
    let mut client = spawn_client();
    let result = client.initialize().await.expect("initialize");
    assert_eq!(result["protocolVersion"], json!("2024-11-05"));
    assert_eq!(result["serverInfo"]["name"], json!("tantra-agent"));
}

#[tokio::test]
async fn lists_tools() {
    let mut client = spawn_client();
    client.initialize().await.expect("initialize");
    let tools = client.list_tools().await.expect("list tools");
    assert!(!tools.is_empty());
    assert!(
        tools
            .iter()
            .any(|t| t["name"] == json!("get_indicator_catalog")),
        "expected get_indicator_catalog in tool list"
    );
}

#[tokio::test]
async fn calls_tool_and_receives_envelope() {
    let mut client = spawn_client();
    client.initialize().await.expect("initialize");
    let result = client
        .call_tool("get_indicator_catalog", json!({}))
        .await
        .expect("call tool");
    assert_eq!(
        result["structuredContent"]["schema"],
        json!("tantra/result/v1")
    );
}

#[tokio::test]
async fn rpc_error_surfaces() {
    let mut client = spawn_client();
    client.initialize().await.expect("initialize");
    let err = client
        .request("bogus_method", json!({}))
        .await
        .expect_err("bogus method must error");
    match err {
        McpClientError::Rpc { code, .. } => assert_eq!(code, -32601),
        other => panic!("expected Rpc error, got {other:?}"),
    }
}

#[tokio::test]
async fn shutdown_terminates() {
    let mut client = spawn_client();
    client.initialize().await.expect("initialize");
    client.shutdown().await.expect("shutdown");
}
