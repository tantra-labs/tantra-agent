//! Runs the Tantra MCP quantitative server over stdio (JSON-RPC 2.0).

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = std::sync::Arc::new(tantra_agent_mcp::McpServer::new());
    tantra_agent_mcp::run_stdio_server(server).await?;
    Ok(())
}
