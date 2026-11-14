//! Economic-calendar conversational chat tool (plan 0007 Track 2 Phase 5).

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;
use tantra_agent_chat::permission::{ToolRisk, ToolTraits};
use tantra_agent_chat::tool::{ChatTool, PlanContext, ToolContext, ToolPlan};
use tantra_agent_core::{ToolError, ToolOutput, ToolSpec};

use crate::ports::calendar::{CalendarReader, EconomicCalendarQuery};

/// Default number of events when the caller passes no limit (§6.5).
pub const DEFAULT_CALENDAR_LIMIT: usize = 20;

/// Maximum number of events returned per call (token budget cap, §6.5).
pub const MAX_CALENDAR_LIMIT: usize = 50;

/// Local RFC3339 parser (mirrors `crate::market::parse_timestamp`; kept private here so the
/// market module's helper does not need to be made `pub(crate)`).
fn parse_ts(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s).ok().map(|dt| dt.to_utc())
}

/// Input arguments for `economic_calendar`.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EconomicCalendarInput {
    /// ISO country code filter (e.g. "IN", "US"). Omit for all countries.
    pub country: Option<String>,
    /// Impact filter — one of "high", "medium", "low". Omit for all impacts.
    pub impact: Option<String>,
    /// Inclusive start of the event window (RFC3339).
    pub start: Option<String>,
    /// Exclusive end of the event window (RFC3339).
    pub end: Option<String>,
    /// Maximum number of events to return (default 20, cap 50).
    pub limit: Option<usize>,
}

/// Chat tool fetching scheduled macro/economic-calendar events (plan 0007 Track 2 Phase 5).
pub struct EconomicCalendarTool {
    calendar: Arc<dyn CalendarReader>,
    spec: ToolSpec,
}

impl EconomicCalendarTool {
    /// Create new economic-calendar tool.
    pub fn new(calendar: Arc<dyn CalendarReader>) -> Self {
        let schema = schemars::schema_for!(EconomicCalendarInput);
        let spec = ToolSpec::new(
            "economic_calendar",
            "Fetch scheduled economic events (central-bank decisions, CPI/GDP releases, earnings seasons) from the macro calendar.",
            serde_json::to_value(schema).unwrap_or_default(),
        );
        Self { calendar, spec }
    }
}

#[async_trait]
impl ChatTool for EconomicCalendarTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn traits(&self) -> ToolTraits {
        ToolTraits::new(ToolRisk::ReadOnly)
    }

    fn plan(&self, input: &Value, _ctx: &PlanContext<'_>) -> Result<ToolPlan, ToolError> {
        let mut scope = String::from("all countries");
        if let Some(country) = input.get("country").and_then(Value::as_str) {
            scope = format!("country `{country}`");
        }
        if let Some(impact) = input.get("impact").and_then(Value::as_str) {
            scope = format!("{scope}, impact `{impact}`");
        }
        Ok(ToolPlan::new(format!(
            "Fetch economic-calendar events for {scope}"
        )))
    }

    async fn execute(&self, input: Value, _ctx: ToolContext) -> Result<ToolOutput, ToolError> {
        let args: EconomicCalendarInput =
            serde_json::from_value(input).map_err(|e| ToolError::InvalidArguments {
                tool: "economic_calendar".to_string(),
                message: format!("schema validation failure: {e}"),
            })?;

        let query = EconomicCalendarQuery {
            country: args.country,
            impact: args.impact,
            start: args.start.as_deref().and_then(parse_ts),
            end: args.end.as_deref().and_then(parse_ts),
            limit: Some(
                args.limit
                    .unwrap_or(DEFAULT_CALENDAR_LIMIT)
                    .min(MAX_CALENDAR_LIMIT),
            ),
        };

        let events = self
            .calendar
            .economic_calendar(&query)
            .await
            .map_err(|e| e.into_tool_error("economic_calendar"))?;

        let events_json: Vec<Value> = events
            .into_iter()
            .map(|ev| {
                serde_json::json!({
                    "id": ev.id,
                    "country": ev.country,
                    "title": ev.title,
                    "currency": ev.currency,
                    "impact": ev.impact,
                    "forecast": ev.forecast,
                    "previous": ev.previous,
                    "actual": ev.actual,
                    "event_date": ev.event_date,
                })
            })
            .collect();

        Ok(ToolOutput::ok(
            "economic_calendar",
            serde_json::json!({
                "events": events_json,
                "count": events_json.len(),
            }),
        ))
    }
}
