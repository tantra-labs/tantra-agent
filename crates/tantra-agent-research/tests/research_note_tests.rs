//! Tests for `research_note_from_verification` (plan 0009 Phase E / gap 0005).

use tantra_agent_report::NoteVerdict;
use tantra_agent_research::{
    HypothesisGenerator, MarketRegime, StatisticalVerifier, research_note_from_verification,
};

#[test]
fn verified_hypothesis_produces_verified_note() {
    let generator = HypothesisGenerator::new();
    let hypothesis = generator
        .generate_for_regime(
            "sma_cross",
            MarketRegime::TrendUp,
            vec!["NSE:NIFTY50-INDEX".into()],
        )
        .expect("hypothesis generation should succeed");

    // Positive in-sample and out-of-sample returns with variance. Constant
    // return vectors have zero variance, which yields a zero Sharpe ratio and
    // therefore a failed verification.
    let in_sample: Vec<f64> = (0..60)
        .map(|i| if i % 2 == 0 { 0.05 } else { -0.01 })
        .collect();
    let out_of_sample: Vec<f64> = (0..60)
        .map(|i| if i % 2 == 0 { 0.04 } else { -0.01 })
        .collect();
    let report = StatisticalVerifier::default().verify_candidate(&in_sample, &out_of_sample, 5);
    assert!(report.passed, "expected verification to pass: {report:?}");

    let note = research_note_from_verification(
        "SMA Cross",
        &hypothesis,
        &report,
        vec!["$tantra/ref/test".into()],
    );

    assert_eq!(note.verdict, NoteVerdict::Verified);
    assert_eq!(note.regime, "trend-up");
    assert!(!note.metrics.is_empty());
    assert!(note.metrics.iter().any(|m| m.name == "DSR"));
    let md = note.to_markdown();
    assert!(md.contains("Verdict: verified"));
    assert!(md.contains("Evidence"));
}

#[test]
fn rejected_hypothesis_lists_reasons() {
    let generator = HypothesisGenerator::new();
    let hypothesis = generator
        .generate_for_regime(
            "sma_cross",
            MarketRegime::TrendUp,
            vec!["NSE:NIFTY50-INDEX".into()],
        )
        .expect("hypothesis generation should succeed");

    let report =
        StatisticalVerifier::default().verify_candidate(&vec![0.0; 60], &vec![-0.01; 60], 5);
    assert!(!report.passed);
    assert!(!report.rejection_reasons.is_empty());

    let note = research_note_from_verification("SMA Cross", &hypothesis, &report, vec![]);

    assert_eq!(note.verdict, NoteVerdict::Rejected);
    let md = note.to_markdown();
    assert!(md.contains("Rejection reasons"));
}
