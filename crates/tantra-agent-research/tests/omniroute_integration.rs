//! Live OmniRoute research-loop integration test.
//!
//! Ignored by default (requires a running OmniRoute / OpenAI-compatible router
//! and `TANTRA_LLM_BASE_URL` / `TANTRA_LLM_MODEL` env vars). Run manually:
//!
//! ```text
//! TANTRA_LLM_BASE_URL=http://localhost:11434/v1 \
//! TANTRA_LLM_MODEL=gpt-4o-mini \
//! cargo test -p tantra-agent-research --features llm -- --ignored
//! ```

use std::sync::Arc;

use rust_decimal_macros::dec;
use tantra_agent_llm::{ConfigCatalog, LlmProvider, ModelCatalog, load_config};
use tantra_agent_research::enhancer::FailureRegimeDiagnostic;
use tantra_agent_research::{MarketRegime, ResearchLlm};

fn mean_reversion_diagnostic() -> FailureRegimeDiagnostic {
    FailureRegimeDiagnostic {
        strategy_name: "intraday_momentum".to_string(),
        failure_regime: MarketRegime::MeanReversion,
        drawdown_pct: dec!(12.4),
        loss_rate: dec!(0.62),
        max_loss_streak: 5,
        loss_clusters: Vec::new(),
        avg_mae: dec!(0.8),
        avg_mfe: dec!(1.1),
        trade_efficiency: dec!(-0.2),
        diagnosis_summary: "Strategy suffers losses in sideways chop".to_string(),
    }
}

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
async fn test_research_llm_suggestion_against_omniroute() {
    let provider = match load_provider() {
        Some(p) => p,
        None => return,
    };
    let research_llm = ResearchLlm::new(provider);

    let diagnostic = mean_reversion_diagnostic();
    let suggestion = research_llm
        .suggest_enhancement(&diagnostic)
        .await
        .expect("suggestion from live router");
    assert!(!suggestion.trim().is_empty());
}
