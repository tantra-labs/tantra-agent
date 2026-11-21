//! `present_view`: pure, read-only layout of the client's right-hand pane.
//!
//! The model references earlier tool results by tool name (`include_tools`) instead of
//! re-typing data; this module only validates the layout and builds the envelope.

use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;

use crate::result::{DisplayBlock, MetricItem, TantraResultEnvelope, ViewKind};

/// Max title length (chars).
pub const MAX_VIEW_TITLE: usize = 120;
/// Max summary length (chars).
pub const MAX_VIEW_SUMMARY: usize = 4000;
/// Max highlight metrics.
pub const MAX_VIEW_HIGHLIGHTS: usize = 12;
/// Max focus symbols.
pub const MAX_VIEW_SYMBOLS: usize = 4;
/// Max included tool names.
pub const MAX_VIEW_INCLUDE_TOOLS: usize = 16;
/// Max length (chars) of each symbol, highlight label/value and tool name.
pub const MAX_VIEW_SHORT: usize = 40;

/// Request for `present_view`.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PresentViewRequest {
    /// Layout to open: analysis, options, backtest, strategy_lab, portfolio, compare or report.
    pub kind: ViewKind,
    /// View title (max 120 chars).
    pub title: String,
    /// Focus symbols (max 4, each max 40 chars).
    #[serde(default)]
    pub symbols: Vec<String>,
    /// Short markdown summary (max 4000 chars). Only state numbers obtained from tools.
    #[serde(default)]
    pub summary: String,
    /// Up to 12 highlight metrics (label/value strings, each max 40 chars).
    #[serde(default)]
    pub highlights: Vec<MetricItem>,
    /// Names of earlier tools (lowercase `[a-z0-9_]+`, max 16) whose results the view shows.
    #[serde(default)]
    pub include_tools: Vec<String>,
}

/// Validation failure for `present_view`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct ViewError {
    /// Human-readable reason.
    pub message: String,
}

fn fail(message: impl Into<String>) -> ViewError {
    ViewError {
        message: message.into(),
    }
}

fn cap(what: &str, value: &str, max: usize) -> Result<(), ViewError> {
    if value.chars().count() > max {
        return Err(fail(format!("{what} exceeds {max} characters")));
    }
    Ok(())
}

/// Validate a request and build the `tantra/result/v1` envelope with one `View` block.
pub fn present_view(
    req: &PresentViewRequest,
) -> Result<TantraResultEnvelope<serde_json::Value>, ViewError> {
    let title = req.title.trim();
    if title.is_empty() {
        return Err(fail("title must not be empty"));
    }
    cap("title", title, MAX_VIEW_TITLE)?;
    cap("summary", &req.summary, MAX_VIEW_SUMMARY)?;
    if req.symbols.len() > MAX_VIEW_SYMBOLS {
        return Err(fail(format!("at most {MAX_VIEW_SYMBOLS} symbols")));
    }
    for s in &req.symbols {
        if s.trim().is_empty() {
            return Err(fail("symbols must not be empty"));
        }
        cap("symbol", s, MAX_VIEW_SHORT)?;
    }
    if req.highlights.len() > MAX_VIEW_HIGHLIGHTS {
        return Err(fail(format!("at most {MAX_VIEW_HIGHLIGHTS} highlights")));
    }
    for h in &req.highlights {
        cap("highlight label", &h.label, MAX_VIEW_SHORT)?;
        cap("highlight value", &h.value, MAX_VIEW_SHORT)?;
    }
    if req.include_tools.len() > MAX_VIEW_INCLUDE_TOOLS {
        return Err(fail(format!(
            "at most {MAX_VIEW_INCLUDE_TOOLS} include_tools"
        )));
    }
    for t in &req.include_tools {
        cap("tool name", t, MAX_VIEW_SHORT)?;
        if t.is_empty()
            || !t
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        {
            return Err(fail(format!(
                "invalid tool name '{t}' (expected [a-z0-9_]+)"
            )));
        }
    }
    let block = DisplayBlock::View {
        kind: req.kind,
        title: title.to_string(),
        symbols: req.symbols.clone(),
        summary: req.summary.clone(),
        highlights: req.highlights.clone(),
        include_tools: req.include_tools.clone(),
    };
    Ok(TantraResultEnvelope::ok(
        format!("present_view: {title}"),
        json!({"kind": req.kind, "title": title}),
    )
    .with_blocks(vec![block]))
}
