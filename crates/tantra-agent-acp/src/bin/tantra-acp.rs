//! `tantra-acp`: serve the Tantra chat agent over ACP on stdin/stdout.

use std::sync::Arc;

use tantra_agent_acp::{AgentHost, StaticCatalog, serve};
use tantra_agent_chat::{InMemorySessionStore, ToolRegistry, TurnConfig};
use tantra_agent_chat_tools::catalog::create_strategy_tools;
use tantra_agent_llm::LlmProvider;
use tantra_agent_llm::candle_gguf::{CandleGgufConfig, CandleGgufProvider};
use tantra_agent_llm::config::openai_provider_from_env;

const SYSTEM_PROMPT: &str = "You are Tantra, an intelligent quantitative finance and algorithmic trading assistant for Indian equities (NSE/BSE).";

fn build_catalog() -> StaticCatalog {
    let mut catalog = StaticCatalog::new();
    if std::env::var("TANTRA_LLM_BASE_URL").is_ok() {
        match openai_provider_from_env() {
            Ok(p) => {
                let id = p.model_name().to_string();
                catalog = catalog.with_model(id.clone(), id, Arc::new(p));
            }
            Err(e) => tracing::warn!("openai-compatible provider unavailable: {e}"),
        }
    }
    let config = CandleGgufConfig::default();
    let p = CandleGgufProvider::new(config);
    let id = p.model_name().to_string();
    catalog = catalog.with_model(id.clone(), id, Arc::new(p));
    catalog
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Logs go to stderr only; stdout carries the ACP stream.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let mut registry = ToolRegistry::new();
    registry.register_all(create_strategy_tools(None));
    let host = Arc::new(AgentHost {
        system_prompt: SYSTEM_PROMPT.to_string(),
        tools: Arc::new(registry.snapshot()),
        store: Arc::new(InMemorySessionStore::new()),
        catalog: Arc::new(build_catalog()),
        turn_config: TurnConfig::default(),
        max_sessions: 16,
    });
    serve(host, tokio::io::stdin(), tokio::io::stdout()).await?;
    Ok(())
}
