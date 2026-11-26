//! Live OmniRoute report-narration integration test.
//!
//! Ignored by default (requires a running OmniRoute / OpenAI-compatible router
//! and `TANTRA_LLM_BASE_URL` / `TANTRA_LLM_MODEL` env vars). Run manually:
//!
//! ```text
//! TANTRA_LLM_BASE_URL=http://localhost:11434/v1 \
//! TANTRA_LLM_MODEL=gpt-4o-mini \
//! cargo test -p tantra-agent-report --features llm -- --ignored
//! ```

use std::sync::Arc;
use tantra_agent_llm::{ConfigCatalog, LlmProvider, ModelCatalog, load_config};
use tantra_agent_report::{Metric, TearsheetReport, narrate_report};

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
async fn test_report_narrate_against_omniroute() {
    let provider = match load_provider() {
        Some(p) => p,
        None => return,
    };

    let mut report = TearsheetReport::new("TCS momentum");
    report.add_metric(Metric::new("Sharpe", "1.42"));
    report.add_metric(Metric::new("Max drawdown", "-8.3%"));

    let narrative = narrate_report(provider.as_ref(), &report)
        .await
        .expect("narrative from live router");
    assert!(!narrative.trim().is_empty());
    assert!(narrative.contains("TCS"), "narrative mentions the strategy");
}
