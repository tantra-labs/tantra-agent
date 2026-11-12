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
use tantra_agent_llm::openai_provider_from_env;

#[tokio::test]
#[ignore = "requires a running OmniRoute router and TANTRA_LLM_* env vars"]
async fn test_chat_turn_round_trips_against_omniroute() {
    let provider = Arc::new(openai_provider_from_env().expect("TANTRA_LLM_* env vars"));
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
