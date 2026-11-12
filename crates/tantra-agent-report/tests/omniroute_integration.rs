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

use tantra_agent_llm::openai_provider_from_env;
use tantra_agent_report::{Metric, TearsheetReport, narrate_report};

#[tokio::test]
#[ignore = "requires a running OmniRoute router and TANTRA_LLM_* env vars"]
async fn test_report_narrate_against_omniroute() {
    let provider = openai_provider_from_env().expect("TANTRA_LLM_* env vars");

    let mut report = TearsheetReport::new("TCS momentum");
    report.add_metric(Metric::new("Sharpe", "1.42"));
    report.add_metric(Metric::new("Max drawdown", "-8.3%"));

    let narrative = narrate_report(&provider, &report)
        .await
        .expect("narrative from live router");
    assert!(!narrative.trim().is_empty());
    assert!(narrative.contains("TCS"), "narrative mentions the strategy");
}
