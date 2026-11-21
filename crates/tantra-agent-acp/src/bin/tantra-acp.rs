//! `tantra-acp`: serve the Tantra chat agent over ACP on stdin/stdout.
//!
//! Usage: `tantra-acp [--config PATH] [--model ID]`. Providers come from the
//! `tantra.jsonc` config file (see `docs/design/provider-config.md`).

use std::path::PathBuf;
use std::sync::Arc;

use tantra_agent_acp::{AgentHost, ModelCatalog, serve};
use tantra_agent_chat::{InMemorySessionStore, ToolRegistry, TurnConfig};
use tantra_agent_chat_tools::catalog::create_strategy_tools;
use tantra_agent_llm::{ConfigCatalog, load_config};

const SYSTEM_PROMPT: &str = "You are Tantra, an intelligent quantitative finance and algorithmic trading assistant for Indian equities (NSE/BSE).";

fn fail(msg: &str) -> ! {
    eprintln!("tantra-acp: {msg}");
    std::process::exit(2);
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Logs go to stderr only; stdout carries the ACP stream.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let mut config_path: Option<PathBuf> = None;
    let mut model: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--config" => match args.next() {
                Some(v) => config_path = Some(PathBuf::from(v)),
                None => fail("--config requires a path"),
            },
            "--model" => match args.next() {
                Some(v) => model = Some(v),
                None => fail("--model requires an id"),
            },
            other => fail(&format!("unknown argument: {other}")),
        }
    }

    let loaded = load_config(config_path.as_deref()).unwrap_or_else(|e| fail(&e.to_string()));
    let mut catalog = ConfigCatalog::build(&loaded, &|k| std::env::var(k).ok())
        .unwrap_or_else(|e| fail(&e.to_string()));
    if let Some(id) = model {
        if catalog.provider(&id).is_err() {
            fail(&format!("unknown model: {id}"));
        }
        catalog = catalog.with_default(id);
    }

    let mut registry = ToolRegistry::new();
    registry.register_all(create_strategy_tools(None));
    let host = Arc::new(AgentHost {
        system_prompt: SYSTEM_PROMPT.to_string(),
        tools: Arc::new(registry.snapshot()),
        store: Arc::new(InMemorySessionStore::new()),
        catalog: Arc::new(catalog),
        turn_config: TurnConfig::default(),
        max_sessions: 16,
    });
    serve(host, tokio::io::stdin(), tokio::io::stdout()).await?;
    Ok(())
}
