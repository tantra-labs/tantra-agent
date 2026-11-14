//! MCP stdio client bridge (plan 0007 Track 6 Phase 4).
//!
//! A minimal JSON-RPC 2.0 client that spawns an MCP server process as a child
//! and talks to it over stdio pipes. Used by integration tests to exercise the
//! real `mcp_stdio_server` binary, and by tooling that needs to drive the
//! Tantra quantitative server out-of-process.

use std::process::Stdio;

use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

use crate::server::RpcMessage;

/// Errors raised by the MCP stdio client.
#[derive(Debug, thiserror::Error)]
pub enum McpClientError {
    /// The child server process could not be spawned.
    #[error("failed to spawn MCP server: {0}")]
    Spawn(String),
    /// An underlying I/O operation failed.
    #[error("io error: {0}")]
    Io(String),
    /// A response line could not be parsed as JSON-RPC.
    #[error("json error: {0}")]
    Json(serde_json::Error),
    /// The server answered with a JSON-RPC error object.
    #[error("server returned error {code}: {message}")]
    Rpc {
        /// JSON-RPC error code (e.g. -32601 method not found).
        code: i32,
        /// Human-readable error message from the server.
        message: String,
    },
    /// The server answered with an unexpected payload shape.
    #[error("unexpected response: {0}")]
    Unexpected(String),
    /// The server closed the stdio pipe (EOF) before answering.
    #[error("server closed the stdio pipe")]
    Closed,
}

/// A spawned MCP server process driven over piped stdio.
pub struct McpClient {
    child: tokio::process::Child,
    stdin: tokio::process::ChildStdin,
    reader: tokio::io::BufReader<tokio::process::ChildStdout>,
    next_id: u64,
}

impl McpClient {
    /// Spawn the MCP server binary as a child process with piped stdin/stdout.
    ///
    /// Stderr is inherited so server diagnostics stay visible on the terminal.
    pub fn spawn(command: &str, args: &[&str]) -> Result<McpClient, McpClientError> {
        let mut child = tokio::process::Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| McpClientError::Spawn(e.to_string()))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| McpClientError::Spawn("stdin was not piped".to_string()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| McpClientError::Spawn("stdout was not piped".to_string()))?;
        Ok(McpClient {
            child,
            stdin,
            reader: tokio::io::BufReader::new(stdout),
            next_id: 0,
        })
    }

    /// Send a JSON-RPC 2.0 request and await the response for its id.
    ///
    /// Responses for other request ids and any notifications are ignored.
    pub async fn request(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, McpClientError> {
        self.next_id += 1;
        let id = self.next_id;
        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        let mut line = serde_json::to_string(&request).map_err(McpClientError::Json)?;
        line.push('\n');
        self.stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|e| McpClientError::Io(e.to_string()))?;
        self.stdin
            .flush()
            .await
            .map_err(|e| McpClientError::Io(e.to_string()))?;

        loop {
            let mut buffer = String::new();
            let bytes_read = self
                .reader
                .read_line(&mut buffer)
                .await
                .map_err(|e| McpClientError::Io(e.to_string()))?;
            if bytes_read == 0 {
                return Err(McpClientError::Closed);
            }
            let msg: RpcMessage =
                serde_json::from_str(buffer.trim()).map_err(McpClientError::Json)?;
            if let RpcMessage::Response {
                id: rid,
                result,
                error,
            } = msg
            {
                if rid == json!(id) {
                    if let Some(err) = error {
                        return Err(McpClientError::Rpc {
                            code: err.code,
                            message: err.message,
                        });
                    }
                    return result
                        .ok_or_else(|| McpClientError::Unexpected("missing result".to_string()));
                }
                // Response for a different request id — keep reading.
            }
            // Notifications and other responses are ignored.
        }
    }

    /// Perform the MCP `initialize` handshake.
    pub async fn initialize(&mut self) -> Result<serde_json::Value, McpClientError> {
        self.request(
            "initialize",
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "tantra-mcp-client",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }),
        )
        .await
    }

    /// List the server's advertised tools.
    pub async fn list_tools(&mut self) -> Result<Vec<serde_json::Value>, McpClientError> {
        let result = self.request("tools/list", json!({})).await?;
        result
            .get("tools")
            .and_then(|v| v.as_array())
            .cloned()
            .ok_or_else(|| {
                McpClientError::Unexpected("tools/list result missing tools array".to_string())
            })
    }

    /// Call an MCP tool by name, returning the full `tools/call` result.
    pub async fn call_tool(
        &mut self,
        name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, McpClientError> {
        self.request(
            "tools/call",
            json!({ "name": name, "arguments": arguments }),
        )
        .await
    }

    /// Request a graceful stop and kill the child process.
    pub async fn shutdown(&mut self) -> Result<(), McpClientError> {
        // The server may not implement `shutdown`; ignore its response either
        // way and terminate the process.
        let _ = self.request("shutdown", json!({})).await;
        self.child
            .kill()
            .await
            .map_err(|e| McpClientError::Io(e.to_string()))?;
        Ok(())
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}
