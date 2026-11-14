//! Structured research notes (`ResearchNote`) with Typst / Markdown rendering
//! (plan 0009 Phase E / gap 0005).

use crate::{Metric, ReportSection};

/// Verification outcome of a research note.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteVerdict {
    /// Hypothesis passed statistical verification.
    Verified,
    /// Hypothesis was rejected by statistical verification.
    Rejected,
    /// Verification has not been run yet.
    Pending,
}

impl std::fmt::Display for NoteVerdict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NoteVerdict::Verified => write!(f, "verified"),
            NoteVerdict::Rejected => write!(f, "rejected"),
            NoteVerdict::Pending => write!(f, "pending"),
        }
    }
}

/// A research note: a falsifiable hypothesis, evidence references, a
/// verification outcome, and quantitative metrics.
#[derive(Debug, Clone, PartialEq)]
pub struct ResearchNote {
    /// Short human-readable title.
    pub title: String,
    /// Market-regime label (e.g. "trend-up").
    pub regime: String,
    /// The falsifiable proposition.
    pub hypothesis: String,
    /// Entry condition rule.
    pub entry_rule: String,
    /// Exit / risk-management rule.
    pub exit_rule: String,
    /// Data / backtest reference URIs backing the note.
    pub evidence_refs: Vec<String>,
    /// Verification outcome.
    pub verdict: NoteVerdict,
    /// Header metrics (reuses [`Metric`]).
    pub metrics: Vec<Metric>,
    /// Free-form sections.
    pub sections: Vec<ReportSection>,
    /// RFC3339 creation timestamp.
    pub created_at: String,
}

impl ResearchNote {
    /// Create an empty note with the given title, regime and hypothesis.
    ///
    /// The verdict defaults to [`NoteVerdict::Pending`] and `created_at` is an
    /// empty string — the caller is responsible for setting the timestamp so
    /// this crate stays free of direct clock access.
    pub fn new(
        title: impl Into<String>,
        regime: impl Into<String>,
        hypothesis: impl Into<String>,
    ) -> Self {
        Self {
            title: title.into(),
            regime: regime.into(),
            hypothesis: hypothesis.into(),
            entry_rule: String::new(),
            exit_rule: String::new(),
            evidence_refs: Vec::new(),
            verdict: NoteVerdict::Pending,
            metrics: Vec::new(),
            sections: Vec::new(),
            created_at: String::new(),
        }
    }

    /// Add a header metric.
    pub fn add_metric(&mut self, metric: Metric) {
        self.metrics.push(metric);
    }

    /// Add a free-form section.
    pub fn add_section(&mut self, section: ReportSection) {
        self.sections.push(section);
    }

    /// Set the verification verdict (builder-style).
    pub fn with_verdict(mut self, verdict: NoteVerdict) -> Self {
        self.verdict = verdict;
        self
    }

    /// Append an evidence reference URI (builder-style).
    pub fn add_evidence(mut self, uri: impl Into<String>) -> Self {
        self.evidence_refs.push(uri.into());
        self
    }

    /// Set the RFC3339 creation timestamp (builder-style).
    pub fn with_created_at(mut self, ts: impl Into<String>) -> Self {
        self.created_at = ts.into();
        self
    }

    /// Render the note as Markdown.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("# {}\n\n", self.title));
        out.push_str(&format!(
            "Verdict: {} · Regime: {}\n\n",
            self.verdict, self.regime
        ));
        if !self.metrics.is_empty() {
            out.push_str("| Metric | Value |\n| :--- | :--- |\n");
            for metric in &self.metrics {
                out.push_str(&format!("| {} | {} |\n", metric.name, metric.value));
            }
            out.push('\n');
        }
        if !self.hypothesis.is_empty() {
            out.push_str(&format!("## Hypothesis\n\n{}\n\n", self.hypothesis));
        }
        if !self.entry_rule.is_empty() {
            out.push_str(&format!("## Entry Rule\n\n{}\n\n", self.entry_rule));
        }
        if !self.exit_rule.is_empty() {
            out.push_str(&format!("## Exit Rule\n\n{}\n\n", self.exit_rule));
        }
        if !self.evidence_refs.is_empty() {
            out.push_str("## Evidence\n\n");
            for uri in &self.evidence_refs {
                out.push_str(&format!("- {uri}\n"));
            }
            out.push('\n');
        }
        for section in &self.sections {
            out.push_str(&format!("## {}\n\n{}\n\n", section.heading, section.body));
        }
        out
    }

    /// Render the note as Typst source.
    pub fn to_typst(&self) -> String {
        let mut out = String::new();
        out.push_str("#set page(paper: \"a4\", margin: 2cm)\n#set text(size: 10pt)\n\n");
        out.push_str(&format!(
            "#align(center)[#text(size: 18pt)[{}]]\n\n",
            self.title
        ));
        out.push_str(&format!(
            "Verdict: {} · Regime: {}\n\n",
            self.verdict, self.regime
        ));
        if !self.metrics.is_empty() {
            out.push_str("#table(\n  columns: 2,\n  [*Metric*], [*Value*],\n");
            for metric in &self.metrics {
                out.push_str(&format!("  [{}], [{}],\n", metric.name, metric.value));
            }
            out.push_str(")\n\n");
        }
        if !self.hypothesis.is_empty() {
            out.push_str(&format!("== Hypothesis\n\n{}\n\n", self.hypothesis));
        }
        if !self.entry_rule.is_empty() {
            out.push_str(&format!("== Entry Rule\n\n{}\n\n", self.entry_rule));
        }
        if !self.exit_rule.is_empty() {
            out.push_str(&format!("== Exit Rule\n\n{}\n\n", self.exit_rule));
        }
        if !self.evidence_refs.is_empty() {
            out.push_str("== Evidence\n\n");
            for uri in &self.evidence_refs {
                out.push_str(&format!("- {uri}\n"));
            }
            out.push('\n');
        }
        for section in &self.sections {
            out.push_str(&format!("== {}\n\n{}\n\n", section.heading, section.body));
        }
        out
    }
}
