//! Live OmniRoute chat integration test.
//!
//! Ignored by default (requires a running OmniRoute / OpenAI-compatible router
//! and `TANTRA_LLM_BASE_URL` / `TANTRA_LLM_MODEL` env vars). Run manually:
//!
//! ```text
//! TANTRA_LLM_BASE_URL=http://localhost:11434/v1 \
//! TANTRA_LLM_MODEL=gpt-4o-mini \
//! cargo test -p tantra-agent-chat -- --ignored
//! ```

use std::sync::Arc;
use std::time::Duration;

use tokio::time::timeout;

use tantra_agent_chat::{
    ChatAgent, InMemorySessionStore, SessionEvent, SessionSelector, ToolRegistry, TurnOutcome,
    run_turn_headless,
};
use tantra_agent_core::{RuntimeMode, SessionScope};
use tantra_agent_llm::{ConfigCatalog, LlmProvider, ModelCatalog, load_config};

fn load_provider() -> Option<Arc<dyn LlmProvider>> {
    let loaded = match load_config(None) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("skipping: {e}");
            return None;
        }
    };
    let catalog = match ConfigCatalog::build(&loaded, &|k| std::env::var(k).ok()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("skipping: {e}");
            return None;
        }
    };
    catalog.provider(&catalog.default_model()).ok()
}

#[tokio::test]
#[ignore = "requires a running gateway and a tantra.jsonc config"]
async fn test_chat_turn_round_trips_against_omniroute() {
    let provider = match load_provider() {
        Some(p) => p,
        None => return,
    };
    let store = Arc::new(InMemorySessionStore::new());
    let agent = ChatAgent::builder(provider, ToolRegistry::new(), store.clone())
        .build()
        .expect("agent build");

    let scope = SessionScope::new("omniroute-integration", RuntimeMode::Paper);
    let (outcome, events) = timeout(
        Duration::from_secs(60),
        run_turn_headless(
            &agent,
            SessionSelector::New(scope),
            "Reply with the single word: hello",
        ),
    )
    .await
    .expect("turn timeout")
    .expect("turn outcome");

    assert_eq!(outcome, TurnOutcome::Completed);
    let has_text = events
        .iter()
        .any(|e| matches!(e, SessionEvent::TextDelta { .. }));
    assert!(has_text, "provider produced at least one text delta");
}
