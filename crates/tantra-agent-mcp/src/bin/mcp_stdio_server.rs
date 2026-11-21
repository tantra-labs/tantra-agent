//! Runs the Tantra MCP quantitative server over stdio (JSON-RPC 2.0).

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut server = tantra_agent_mcp::McpServer::new();
    if let Some(client) = tantra_agent_mcp::RuntimeLibraryClient::from_env() {
        let client = std::sync::Arc::new(client);
        server = server
            .with_library_port(client.clone())
            .with_loop_port(client);
    }
    let server = std::sync::Arc::new(server);
    tantra_agent_mcp::run_stdio_server(server).await?;
    Ok(())
}
