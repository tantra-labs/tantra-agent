//! Conversational `ReadOnly` tools for `Experimental-Analysis` (plan 0016):
//! - `experimental_study_summary`: Empirical period returns, Markov streak prediction,
//!   80%/90%/95% strike-sizing levels & conditional forward metrics.
//! - `historical_outlier_news`: Date-bounded `[window_start - 2d, window_end + 1d]`
//!   news archive + sovereign/macro calendar catalyst lookup with in-memory bundle caching.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::{DateTime, Duration, NaiveDate, Utc};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;
use tantra_agent_chat::permission::{ToolRisk, ToolTraits};
use tantra_agent_chat::tool::{ChatTool, PlanContext, ToolContext, ToolPlan};
use tantra_agent_core::{ToolError, ToolOutput, ToolSpec};

use crate::market::parse_instrument;
use crate::ports::calendar::{CalendarReader, EconomicCalendarQuery};
use crate::ports::content::ContentReader;
use crate::ports::error::PortError;
use crate::ports::study::StudyReader;

const MAX_NEWS_LIMIT: usize = 50;
const DEFAULT_NEWS_LIMIT: usize = 20;

fn parse_naive_date(raw: &str, field: &str, tool: &str) -> Result<NaiveDate, ToolError> {
    NaiveDate::parse_from_str(raw.trim(), "%Y-%m-%d").map_err(|e| ToolError::InvalidArguments {
        tool: tool.to_string(),
        message: format!("invalid {field} `{raw}` (expected YYYY-MM-DD): {e}"),
    })
}

fn default_keywords_for_asset_class(asset_class: &str, symbol: &str) -> String {
    match asset_class.trim().to_ascii_lowercase().as_str() {
        "commodity" | "commodities" => "crude oil opec gold fed macro".to_string(),
        "forex" | "fx" => "rbi fed currency dollar usd inr macro".to_string(),
        "index" | "indices" => format!("{symbol} market economy finance global crash"),
        "etf" | "etfs" => format!("{symbol} etf flows macro index"),
        _ => format!("{symbol} earnings macro corporate event"),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. experimental_study_summary
// ─────────────────────────────────────────────────────────────────────────────

/// Input arguments for `experimental_study_summary`.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExperimentalStudySummaryInput {
    /// Instrument symbol (e.g. `"NSE:NIFTY50"`, `"NSE:RELIANCE"`, `"MCX:GOLD"`).
    pub instrument: String,
    /// Optional asset class hint (`"index"`, `"fno_stock"`, `"etf"`, `"equity"`, `"forex"`, `"commodity"`).
    pub asset_class: Option<String>,
    /// Optional period schedule (`"weekly_expiry"`, `"week_plus_one"`, `"monthly_expiry"`, `"calendar_weekly"`, `"calendar_monthly"`, `"front_month_cycle"`, `"mcx_expiry_cycle"`, `"daily"`).
    pub period: Option<String>,
    /// Optional return formula (`"open_to_close"`, `"high_to_low"`, `"close_to_close"`).
    pub calc_type: Option<String>,
    /// Optional inclusive start date (`YYYY-MM-DD`).
    pub start_date: Option<String>,
    /// Optional inclusive end date (`YYYY-MM-DD`).
    pub end_date: Option<String>,
}

/// Read-only chat tool querying `Experimental-Analysis` statistical KPIs, Markov probabilities & levels.
pub struct ExperimentalStudySummaryTool {
    study: Option<Arc<dyn StudyReader>>,
    spec: ToolSpec,
}

impl ExperimentalStudySummaryTool {
    /// Creates `experimental_study_summary` backed by an optional [`StudyReader`].
    pub fn new(study: Option<Arc<dyn StudyReader>>) -> Self {
        let schema = schemars::schema_for!(ExperimentalStudySummaryInput);
        let spec = ToolSpec::new(
            "experimental_study_summary",
            "Evaluate multi-asset period return KPIs, 6-state Markov next-streak probabilities, 80%/90%/95% probability-based strike levels, and 90% survivability outlier bounds.",
            serde_json::to_value(schema).unwrap_or_default(),
        );
        Self { study, spec }
    }
}

#[async_trait]
impl ChatTool for ExperimentalStudySummaryTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn traits(&self) -> ToolTraits {
        ToolTraits::new(ToolRisk::ReadOnly)
    }

    fn plan(&self, input: &Value, _ctx: &PlanContext<'_>) -> Result<ToolPlan, ToolError> {
        let inst = input
            .get("instrument")
            .and_then(Value::as_str)
            .unwrap_or("instrument");
        let period = input
            .get("period")
            .and_then(Value::as_str)
            .unwrap_or("default schedule");
        Ok(ToolPlan::new(format!(
            "Run Experimental-Analysis study summary for `{inst}` ({period})"
        )))
    }

    async fn execute(&self, input: Value, _ctx: ToolContext) -> Result<ToolOutput, ToolError> {
        let args: ExperimentalStudySummaryInput =
            serde_json::from_value(input).map_err(|e| ToolError::InvalidArguments {
                tool: "experimental_study_summary".to_string(),
                message: format!("schema validation failure: {e}"),
            })?;

        let inst = parse_instrument(&args.instrument)?;
        let start_date = args
            .start_date
            .as_deref()
            .map(|s| parse_naive_date(s, "start_date", "experimental_study_summary"))
            .transpose()?;
        let end_date = args
            .end_date
            .as_deref()
            .map(|s| parse_naive_date(s, "end_date", "experimental_study_summary"))
            .transpose()?;
        if let (Some(s), Some(e)) = (start_date, end_date)
            && s >= e
        {
            return Err(ToolError::InvalidArguments {
                tool: "experimental_study_summary".to_string(),
                message: "start_date must be strictly before end_date".to_string(),
            });
        }

        let Some(reader) = &self.study else {
            return Err(PortError::unavailable(
                "experimental study reader is not wired in this runtime context; use POST /api/v2/experimental-analysis/returns or attach FeedStudyReader",
            )
            .into_tool_error("experimental_study_summary"));
        };

        let summary = reader
            .returns_study_summary(
                &inst,
                args.asset_class.as_deref(),
                args.period.as_deref(),
                args.calc_type.as_deref(),
                start_date,
                end_date,
            )
            .await
            .map_err(|e| e.into_tool_error("experimental_study_summary"))?;

        Ok(ToolOutput::ok(
            "experimental_study_summary",
            serde_json::to_value(summary).unwrap_or_default(),
        ))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. historical_outlier_news
// ─────────────────────────────────────────────────────────────────────────────

/// Input arguments for `historical_outlier_news`.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HistoricalOutlierNewsInput {
    /// Instrument symbol (e.g. `"NSE:NIFTY50"`, `"NSE:RELIANCE"`, `"MCX:GOLD"`).
    pub instrument: String,
    /// Asset class hint (`"index"`, `"fno_stock"`, `"etf"`, `"equity"`, `"forex"`, `"commodity"`).
    pub asset_class: Option<String>,
    /// Outlier period start date (`YYYY-MM-DD`).
    pub window_start: String,
    /// Outlier period end date (`YYYY-MM-DD`).
    pub window_end: String,
    /// Optional space-separated search keywords (defaults to asset-class macro/catalyst terms).
    pub keywords: Option<String>,
    /// Maximum number of catalyst items to return (`1..=50`, default `20`).
    pub limit: Option<usize>,
}

type CacheKey = (String, String, String, String, String);

/// Read-only chat tool retrieving date-bounded `[window_start - 2d, window_end + 1d]`
/// archival news headlines & macro calendar catalysts for outlier periods.
pub struct HistoricalOutlierNewsTool {
    content: Arc<dyn ContentReader>,
    calendar: Arc<dyn CalendarReader>,
    cache: Mutex<HashMap<CacheKey, Value>>,
    spec: ToolSpec,
}

impl HistoricalOutlierNewsTool {
    /// Creates `historical_outlier_news` over read-only content & calendar ports.
    pub fn new(content: Arc<dyn ContentReader>, calendar: Arc<dyn CalendarReader>) -> Self {
        let schema = schemars::schema_for!(HistoricalOutlierNewsInput);
        let spec = ToolSpec::new(
            "historical_outlier_news",
            "Retrieve dated historical news headlines and macro calendar releases around an outlier period window [window_start - 2d, window_end + 1d]. Never fabricates headlines when archives are empty or offline.",
            serde_json::to_value(schema).unwrap_or_default(),
        );
        Self {
            content,
            calendar,
            cache: Mutex::new(HashMap::new()),
            spec,
        }
    }
}

fn matches_any_keyword(text: &str, tokens: &[String]) -> bool {
    if tokens.is_empty() {
        return true;
    }
    let lower = text.to_ascii_lowercase();
    tokens.iter().any(|tok| lower.contains(tok))
}

#[async_trait]
impl ChatTool for HistoricalOutlierNewsTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn traits(&self) -> ToolTraits {
        ToolTraits::new(ToolRisk::ReadOnly)
    }

    fn plan(&self, input: &Value, _ctx: &PlanContext<'_>) -> Result<ToolPlan, ToolError> {
        let inst = input
            .get("instrument")
            .and_then(Value::as_str)
            .unwrap_or("instrument");
        let start = input
            .get("window_start")
            .and_then(Value::as_str)
            .unwrap_or("start");
        let end = input
            .get("window_end")
            .and_then(Value::as_str)
            .unwrap_or("end");
        Ok(ToolPlan::new(format!(
            "Search historical news & macro catalysts for `{inst}` around {start}..{end}"
        )))
    }

    async fn execute(&self, input: Value, _ctx: ToolContext) -> Result<ToolOutput, ToolError> {
        let args: HistoricalOutlierNewsInput =
            serde_json::from_value(input).map_err(|e| ToolError::InvalidArguments {
                tool: "historical_outlier_news".to_string(),
                message: format!("schema validation failure: {e}"),
            })?;

        let inst = parse_instrument(&args.instrument)?;
        let window_start = parse_naive_date(
            &args.window_start,
            "window_start",
            "historical_outlier_news",
        )?;
        let window_end =
            parse_naive_date(&args.window_end, "window_end", "historical_outlier_news")?;
        if window_start > window_end {
            return Err(ToolError::InvalidArguments {
                tool: "historical_outlier_news".to_string(),
                message: "window_start cannot be after window_end".to_string(),
            });
        }

        let asset_class = args
            .asset_class
            .as_deref()
            .unwrap_or("equity")
            .to_ascii_lowercase();
        let keywords = args
            .keywords
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| default_keywords_for_asset_class(&asset_class, &inst.symbol));

        let cache_key: CacheKey = (
            inst.to_string(),
            asset_class.clone(),
            window_start.to_string(),
            window_end.to_string(),
            keywords.to_ascii_lowercase(),
        );
        if let Ok(guard) = self.cache.lock()
            && let Some(cached) = guard.get(&cache_key)
        {
            return Ok(ToolOutput::ok("historical_outlier_news", cached.clone()));
        }

        let expanded_start_date = window_start - Duration::days(2);
        let expanded_end_date = window_end + Duration::days(1);
        let start_utc: DateTime<Utc> = expanded_start_date
            .and_hms_opt(0, 0, 0)
            .expect("valid start midnight")
            .and_utc();
        let end_utc: DateTime<Utc> = expanded_end_date
            .and_hms_opt(23, 59, 59)
            .expect("valid end second")
            .and_utc();

        let tokens: Vec<String> = keywords
            .split(|c: char| !c.is_alphanumeric())
            .map(|t| t.trim().to_ascii_lowercase())
            .filter(|t| t.len() >= 2)
            .collect();
        let limit = args.limit.unwrap_or(DEFAULT_NEWS_LIMIT).min(MAX_NEWS_LIMIT);

        let news_res = self.content.news_search(None, None, Some(250)).await;
        let cal_res = self
            .calendar
            .economic_calendar(&EconomicCalendarQuery {
                country: None,
                impact: None,
                start: Some(start_utc),
                end: Some(end_utc + Duration::seconds(1)),
                limit: Some(100),
            })
            .await;

        let mut news_err: Option<PortError> = None;
        let mut cal_err: Option<PortError> = None;
        let mut items: Vec<Value> = Vec::new();
        let inst_upper = inst.to_string().to_ascii_uppercase();
        let sym_upper = inst.symbol.to_ascii_uppercase();

        match news_res {
            Ok(articles) => {
                for a in articles {
                    let pub_date = a.published_at.date_naive();
                    if pub_date < expanded_start_date || pub_date > expanded_end_date {
                        continue;
                    }
                    let mentions_inst = a.instruments.iter().any(|tag| {
                        let up = tag.to_ascii_uppercase();
                        up == inst_upper || up.contains(&sym_upper)
                    });
                    let haystack = format!(
                        "{} {} {}",
                        a.title,
                        a.summary.as_deref().unwrap_or(""),
                        a.instruments.join(" ")
                    );
                    if mentions_inst || matches_any_keyword(&haystack, &tokens) {
                        items.push(serde_json::json!({
                            "kind": "news",
                            "date": pub_date.to_string(),
                            "published_at": a.published_at.to_rfc3339(),
                            "title": a.title,
                            "source": a.source,
                            "url": a.url,
                            "summary": a.summary,
                            "instruments": a.instruments,
                        }));
                    }
                }
            }
            Err(e) => news_err = Some(e),
        }

        match cal_res {
            Ok(events) => {
                for ev in events {
                    let ev_date = ev.event_date.date_naive();
                    if ev_date < expanded_start_date || ev_date > expanded_end_date {
                        continue;
                    }
                    let summary = format!(
                        "{} ({}) impact={} actual={} forecast={} prior={}",
                        ev.country,
                        ev.currency.as_deref().unwrap_or(""),
                        ev.impact,
                        ev.actual.as_deref().unwrap_or("—"),
                        ev.forecast.as_deref().unwrap_or("—"),
                        ev.previous.as_deref().unwrap_or("—"),
                    );
                    items.push(serde_json::json!({
                        "kind": "macro_calendar",
                        "date": ev_date.to_string(),
                        "published_at": ev.event_date.to_rfc3339(),
                        "title": ev.title,
                        "source": format!("Macro Calendar ({})", ev.country),
                        "url": serde_json::Value::Null,
                        "summary": summary,
                        "instruments": vec![inst.to_string()],
                    }));
                }
            }
            Err(e) => cal_err = Some(e),
        }

        if let (Some(ne), Some(_ce)) = (&news_err, &cal_err)
            && items.is_empty()
        {
            return Err(ne.clone().into_tool_error("historical_outlier_news"));
        }

        items.sort_by(|a, b| b["published_at"].as_str().cmp(&a["published_at"].as_str()));
        items.truncate(limit);

        let payload = serde_json::json!({
            "instrument": inst.to_string(),
            "asset_class": asset_class,
            "expanded_window": {
                "start": expanded_start_date.to_string(),
                "end": expanded_end_date.to_string(),
            },
            "keywords": keywords,
            "items": items,
            "count": items.len(),
            "news_archive_available": news_err.is_none(),
            "macro_calendar_available": cal_err.is_none(),
        });

        if let Ok(mut guard) = self.cache.lock() {
            guard.insert(cache_key, payload.clone());
        }

        Ok(ToolOutput::ok("historical_outlier_news", payload))
    }
}
