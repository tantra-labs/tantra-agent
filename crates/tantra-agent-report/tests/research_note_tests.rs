//! Tests for `ResearchNote` Markdown / Typst rendering (plan 0009 Phase E / gap 0005).

use tantra_agent_report::{Metric, NoteVerdict, ReportSection, ResearchNote};

#[test]
fn markdown_includes_title_verdict_and_metrics() {
    let mut note = ResearchNote::new("Mean Reversion Test", "mean-reversion", "RSI bounce");
    note.add_metric(Metric::new("DSR", "0.9910"));
    note.add_section(ReportSection::new(
        "Conclusion",
        "Hypothesis holds out of sample.",
    ));
    let note = note.with_verdict(NoteVerdict::Verified);

    let md = note.to_markdown();
    assert!(md.contains("Mean Reversion Test"));
    assert!(md.contains("Verdict: verified"));
    assert!(md.contains("DSR"));
    assert!(md.contains("## Conclusion"));
}

#[test]
fn typst_includes_preamble_and_title() {
    let note = ResearchNote::new("Mean Reversion Test", "mean-reversion", "RSI bounce");
    let typst = note.to_typst();
    assert!(typst.contains("#set page"));
    assert!(typst.contains("Mean Reversion Test"));
}

#[test]
fn verdict_display() {
    assert_eq!(NoteVerdict::Verified.to_string(), "verified");
    assert_eq!(NoteVerdict::Rejected.to_string(), "rejected");
    assert_eq!(NoteVerdict::Pending.to_string(), "pending");
}

#[test]
fn builder_pattern() {
    let note = ResearchNote::new("Builder", "volatility-breakout", "Breakout proposition")
        .with_verdict(NoteVerdict::Rejected)
        .with_created_at("2026-11-13T10:00:00Z")
        .add_evidence("$tantra/ref/001");

    assert_eq!(note.verdict, NoteVerdict::Rejected);
    assert_eq!(note.created_at, "2026-11-13T10:00:00Z");
    assert_eq!(note.evidence_refs, vec!["$tantra/ref/001".to_string()]);
}
