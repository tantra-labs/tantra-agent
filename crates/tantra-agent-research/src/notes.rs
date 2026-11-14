//! Research notes produced from the research loop (plan 0009 Phase E / gap 0005).
//!
//! Bridges [`crate::hypothesis::Hypothesis`] and
//! [`crate::verification::StatisticalVerificationReport`] into a renderable
//! [`tantra_agent_report::ResearchNote`].

use tantra_agent_report::{Metric, NoteVerdict, ReportSection, ResearchNote};

use crate::hypothesis::Hypothesis;
use crate::verification::StatisticalVerificationReport;

/// Build a [`ResearchNote`] from a hypothesis and its statistical verification.
pub fn research_note_from_verification(
    title: impl Into<String>,
    hypothesis: &Hypothesis,
    report: &StatisticalVerificationReport,
    evidence_refs: Vec<String>,
) -> ResearchNote {
    let mut note = ResearchNote::new(
        title,
        hypothesis.regime.to_string(),
        hypothesis.description.clone(),
    );
    note.entry_rule = hypothesis.entry_rule.clone();
    note.exit_rule = hypothesis.exit_rule.clone();

    note.add_metric(Metric::new(
        "DSR",
        format!("{:.4}", report.deflated_sharpe_ratio),
    ));
    note.add_metric(Metric::new(
        "PBO",
        format!("{:.4}", report.probability_of_overfitting),
    ));
    note.add_metric(Metric::new(
        "WFE",
        format!("{:.4}", report.walk_forward_efficiency),
    ));
    note.add_metric(Metric::new(
        "IS Sharpe",
        format!("{:.2}", report.in_sample_sharpe),
    ));
    note.add_metric(Metric::new(
        "OOS Sharpe",
        format!("{:.2}", report.out_of_sample_sharpe),
    ));
    note.add_metric(Metric::new("Trials", report.trials_count.to_string()));

    note.add_section(ReportSection::new(
        "Statistical verification",
        format!(
            "In-sample Sharpe {:.2}, out-of-sample Sharpe {:.2}, walk-forward efficiency {:.4}, \
             deflated Sharpe ratio {:.4}, probability of backtest overfitting {:.4} across {} trials.",
            report.in_sample_sharpe,
            report.out_of_sample_sharpe,
            report.walk_forward_efficiency,
            report.deflated_sharpe_ratio,
            report.probability_of_overfitting,
            report.trials_count
        ),
    ));

    if !report.rejection_reasons.is_empty() {
        let reasons = report
            .rejection_reasons
            .iter()
            .map(|reason| format!("- {reason}"))
            .collect::<Vec<_>>()
            .join("\n");
        note.add_section(ReportSection::new("Rejection reasons", reasons));
    }

    let verdict = if report.passed {
        NoteVerdict::Verified
    } else {
        NoteVerdict::Rejected
    };
    let mut note = note.with_verdict(verdict);
    for uri in evidence_refs {
        note = note.add_evidence(uri);
    }
    note.with_created_at(hypothesis.created_at.to_rfc3339())
}
