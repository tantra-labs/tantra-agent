//! Strategy loop port: typed, bounded boundary to the runtime's generic strategy
//! evaluate / sweep / expiry-hedge studies, plus the pure `chart_scene_annotate` packager.
//!
//! Enum-like fields (`asset_class`, `timeframe`, modes) stay `String`; the runtime validates
//! them and the agent must not fork runtime enums. Money-like values are `Decimal`.
//! Everything returned by the runtime is untrusted data.

use std::collections::BTreeMap;

use rust_decimal::Decimal;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

use crate::result::{DisplayBlock, MetricItem, TantraResultEnvelope};

/// Maximum values per sweep grid.
pub const MAX_GRID_VALUES: usize = 16;
/// Maximum total sweep candidates.
pub const MAX_SWEEP_CANDIDATES: usize = 64;
/// Maximum entries in a scene time axis.
pub const MAX_TIME_AXIS: usize = 5000;

/// Classification of a strategy loop port failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrategyLoopPortErrorKind {
    /// Runtime not reachable / not configured.
    Unavailable,
    /// Request failed validation.
    InvalidRequest,
    /// Requested resource does not exist.
    NotFound,
    /// Runtime saturated; retry later.
    Busy,
    /// Call exceeded its deadline.
    Timeout,
    /// Upstream returned an error.
    Upstream,
}

/// Typed strategy loop port error.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
#[error("strategy loop port error ({kind:?}): {message}")]
pub struct StrategyLoopPortError {
    /// Failure class.
    pub kind: StrategyLoopPortErrorKind,
    /// Human-readable message.
    pub message: String,
}

impl StrategyLoopPortError {
    fn make(kind: StrategyLoopPortErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
    /// Runtime unavailable.
    pub fn unavailable(message: impl Into<String>) -> Self {
        Self::make(StrategyLoopPortErrorKind::Unavailable, message)
    }
    /// Invalid request.
    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::make(StrategyLoopPortErrorKind::InvalidRequest, message)
    }
    /// Not found.
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::make(StrategyLoopPortErrorKind::NotFound, message)
    }
    /// Busy.
    pub fn busy(message: impl Into<String>) -> Self {
        Self::make(StrategyLoopPortErrorKind::Busy, message)
    }
    /// Timeout.
    pub fn timeout(message: impl Into<String>) -> Self {
        Self::make(StrategyLoopPortErrorKind::Timeout, message)
    }
    /// Upstream error.
    pub fn upstream(message: impl Into<String>) -> Self {
        Self::make(StrategyLoopPortErrorKind::Upstream, message)
    }
}

impl From<crate::library::LibraryPortError> for StrategyLoopPortError {
    fn from(e: crate::library::LibraryPortError) -> Self {
        use crate::library::LibraryPortErrorKind as L;
        use StrategyLoopPortErrorKind as K;
        let kind = match e.kind {
            L::Unavailable => K::Unavailable,
            L::InvalidRequest => K::InvalidRequest,
            L::NotFound => K::NotFound,
            L::Busy => K::Busy,
            L::Timeout => K::Timeout,
            L::Upstream => K::Upstream,
        };
        Self::make(kind, e.message)
    }
}

fn err(m: impl Into<String>) -> StrategyLoopPortError {
    StrategyLoopPortError::invalid_request(m)
}

fn non_empty(name: &str, v: &str) -> Result<(), StrategyLoopPortError> {
    if v.trim().is_empty() || v.len() > 128 {
        return Err(err(format!("{name} must be 1..=128 non-blank characters")));
    }
    Ok(())
}

/// Strict `YYYY-MM-DD` check.
fn valid_date(name: &str, v: &str) -> Result<(), StrategyLoopPortError> {
    chrono::NaiveDate::parse_from_str(v, "%Y-%m-%d")
        .ok()
        .filter(|_| v.len() == 10)
        .map(|_| ())
        .ok_or_else(|| err(format!("{name} must be YYYY-MM-DD, got '{v}'")))
}

fn positive(name: &str, v: &Option<Decimal>) -> Result<(), StrategyLoopPortError> {
    match v {
        Some(d) if *d <= Decimal::ZERO => Err(err(format!("{name} must be > 0"))),
        _ => Ok(()),
    }
}

fn positive_usize(name: &str, v: &Option<usize>) -> Result<(), StrategyLoopPortError> {
    match v {
        Some(0) => Err(err(format!("{name} must be > 0"))),
        _ => Ok(()),
    }
}

/// Evaluate request (`POST /api/v2/studies/strategy-loop/evaluate`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StrategyLoopEvalRequest {
    /// Strategy identifier.
    pub strategy_id: String,
    /// Instrument identifier.
    pub instrument_id: String,
    /// Asset class: index | fno_stock | etf | equity | forex | commodity.
    pub asset_class: String,
    /// Start date `YYYY-MM-DD`.
    pub start_date: String,
    /// End date `YYYY-MM-DD`.
    pub end_date: String,
    /// Bar timeframe (e.g. `1d`, `15m`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeframe: Option<String>,
    /// Session preset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_preset: Option<String>,
    /// Entry trigger mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry_trigger_mode: Option<String>,
    /// Stop policy mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_policy_mode: Option<String>,
    /// Stop-loss buffer in ATR multiples.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sl_buffer_atr: Option<Decimal>,
    /// Holding horizon in bars.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub horizon_bars: Option<usize>,
    /// Target in R multiples.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_r: Option<Decimal>,
    /// ATR period in bars.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atr_period: Option<usize>,
    /// Partial-exit R multiple.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partial_r: Option<Decimal>,
    /// Runner R multiple.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runner_r: Option<Decimal>,
    /// Time stop in bars.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_stop_bars: Option<usize>,
    /// Expiry horizon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expiry_horizon: Option<String>,
    /// Lot size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lot_size: Option<Decimal>,
    /// Tick size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tick_size: Option<Decimal>,
    /// Capital.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capital: Option<Decimal>,
    /// Existing runtime session id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Maximum chart bars returned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_chart_bars: Option<usize>,
}

impl StrategyLoopEvalRequest {
    /// Enforce bounds before crossing the port.
    pub fn validate(&self) -> Result<(), StrategyLoopPortError> {
        non_empty("strategy_id", &self.strategy_id)?;
        non_empty("instrument_id", &self.instrument_id)?;
        non_empty("asset_class", &self.asset_class)?;
        valid_date("start_date", &self.start_date)?;
        valid_date("end_date", &self.end_date)?;
        if self.start_date > self.end_date {
            return Err(err("start_date must not be after end_date"));
        }
        positive("sl_buffer_atr", &self.sl_buffer_atr)?;
        positive("target_r", &self.target_r)?;
        positive("partial_r", &self.partial_r)?;
        positive("runner_r", &self.runner_r)?;
        positive("lot_size", &self.lot_size)?;
        positive("tick_size", &self.tick_size)?;
        positive("capital", &self.capital)?;
        positive_usize("horizon_bars", &self.horizon_bars)?;
        positive_usize("atr_period", &self.atr_period)?;
        positive_usize("time_stop_bars", &self.time_stop_bars)?;
        positive_usize("max_chart_bars", &self.max_chart_bars)?;
        if matches!(self.max_chart_bars, Some(n) if n > MAX_TIME_AXIS) {
            return Err(err(format!("max_chart_bars must be <= {MAX_TIME_AXIS}")));
        }
        Ok(())
    }
}

/// Sweep grids.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StrategyLoopBounds {
    /// Stop buffer grid (ATR multiples).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sl_buffer_atr_grid: Option<Vec<Decimal>>,
    /// Horizon grid (bars).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub horizon_bars_grid: Option<Vec<usize>>,
    /// Target grid (R multiples).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_r_grid: Option<Vec<Decimal>>,
    /// Candidate cap (runtime clamps to 1..=64).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_candidates: Option<usize>,
}

impl StrategyLoopBounds {
    /// Enforce grid and candidate caps.
    pub fn validate(&self) -> Result<(), StrategyLoopPortError> {
        let mut total: usize = 1;
        let mut any = false;
        let mut lens = Vec::new();
        if let Some(g) = &self.sl_buffer_atr_grid {
            if g.iter().any(|d| *d <= Decimal::ZERO) {
                return Err(err("sl_buffer_atr_grid values must be > 0"));
            }
            lens.push(("sl_buffer_atr_grid", g.len()));
        }
        if let Some(g) = &self.horizon_bars_grid {
            if g.contains(&0) {
                return Err(err("horizon_bars_grid values must be > 0"));
            }
            lens.push(("horizon_bars_grid", g.len()));
        }
        if let Some(g) = &self.target_r_grid {
            if g.iter().any(|d| *d <= Decimal::ZERO) {
                return Err(err("target_r_grid values must be > 0"));
            }
            lens.push(("target_r_grid", g.len()));
        }
        for (name, n) in lens {
            if n > MAX_GRID_VALUES {
                return Err(err(format!("{name} exceeds {MAX_GRID_VALUES} values")));
            }
            any = true;
            total = total.saturating_mul(n.max(1));
        }
        if any && total > MAX_SWEEP_CANDIDATES {
            return Err(err(format!(
                "sweep would produce {total} candidates; maximum is {MAX_SWEEP_CANDIDATES}"
            )));
        }
        if let Some(m) = self.max_candidates {
            if m == 0 || m > MAX_SWEEP_CANDIDATES {
                return Err(err(format!(
                    "max_candidates must be within 1..={MAX_SWEEP_CANDIDATES}"
                )));
            }
        }
        Ok(())
    }
}

/// Sweep request (`POST /api/v2/studies/strategy-loop/sweep`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StrategyLoopSweepRequest {
    /// Base evaluation request.
    pub base: StrategyLoopEvalRequest,
    /// Sweep bounds.
    pub bounds: StrategyLoopBounds,
}

impl StrategyLoopSweepRequest {
    /// Validate base and bounds.
    pub fn validate(&self) -> Result<(), StrategyLoopPortError> {
        self.base.validate()?;
        self.bounds.validate()
    }
}

/// Expiry hedge recommendation request (`POST .../recommend-hedge`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StrategyHedgeRequest {
    /// Instrument identifier.
    pub instrument_id: String,
    /// Asset class: index | fno_stock | etf | equity | forex | commodity.
    pub asset_class: String,
    /// Strategy identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strategy_id: Option<String>,
    /// Start date `YYYY-MM-DD`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    /// End date `YYYY-MM-DD`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_date: Option<String>,
    /// Bar timeframe.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeframe: Option<String>,
    /// Trade direction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    /// Expiry horizon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expiry_horizon: Option<String>,
    /// Stop buffer in ATR multiples.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sl_buffer_atr: Option<Decimal>,
    /// Holding horizon in bars.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub horizon_bars: Option<usize>,
    /// ATR period.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atr_period: Option<usize>,
    /// Lot size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lot_size: Option<Decimal>,
    /// Tick size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tick_size: Option<Decimal>,
    /// Capital.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capital: Option<Decimal>,
}

impl StrategyHedgeRequest {
    /// Enforce bounds before crossing the port.
    pub fn validate(&self) -> Result<(), StrategyLoopPortError> {
        non_empty("instrument_id", &self.instrument_id)?;
        non_empty("asset_class", &self.asset_class)?;
        if let Some(s) = &self.strategy_id {
            non_empty("strategy_id", s)?;
        }
        if let Some(d) = &self.start_date {
            valid_date("start_date", d)?;
        }
        if let Some(d) = &self.end_date {
            valid_date("end_date", d)?;
        }
        if let (Some(a), Some(b)) = (&self.start_date, &self.end_date) {
            if a > b {
                return Err(err("start_date must not be after end_date"));
            }
        }
        positive("sl_buffer_atr", &self.sl_buffer_atr)?;
        positive("lot_size", &self.lot_size)?;
        positive("tick_size", &self.tick_size)?;
        positive("capital", &self.capital)?;
        positive_usize("horizon_bars", &self.horizon_bars)?;
        positive_usize("atr_period", &self.atr_period)?;
        Ok(())
    }
}

/// Port to the runtime's strategy loop studies.
#[async_trait::async_trait]
pub trait StrategyLoopPort: Send + Sync {
    /// Evaluate a strategy over a date range.
    async fn evaluate(&self, req: StrategyLoopEvalRequest) -> Result<Value, StrategyLoopPortError>;
    /// Sweep bounded parameter grids.
    async fn sweep(&self, req: StrategyLoopSweepRequest) -> Result<Value, StrategyLoopPortError>;
    /// Recommend an expiry hedge.
    async fn recommend_hedge(
        &self,
        req: StrategyHedgeRequest,
    ) -> Result<Value, StrategyLoopPortError>;
}

/// Input of the pure `chart_scene_annotate` tool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SceneAnnotateRequest {
    /// Scene title.
    pub title: String,
    /// Instrument symbol.
    pub symbol: String,
    /// Timeframe.
    pub timeframe: String,
    /// Overlay scene object with arrays `signals, bands, stops, indicators, hints, hedges`.
    pub scene: Value,
    /// Optional bar timestamps (unix seconds), at most 5000.
    #[serde(default)]
    pub time_axis: Vec<i64>,
    /// Optional KPI label to value map.
    #[serde(default)]
    pub kpis: BTreeMap<String, String>,
    /// Optional free-text note.
    #[serde(default)]
    pub notes: Option<String>,
}

/// Scene arrays every overlay scene must carry.
pub const SCENE_ARRAYS: [&str; 6] = ["signals", "bands", "stops", "indicators", "hints", "hedges"];

/// Package a scene into a result envelope with an `OverlayScene` display block.
pub fn scene_annotation(
    input: &SceneAnnotateRequest,
) -> Result<TantraResultEnvelope<Value>, StrategyLoopPortError> {
    non_empty("title", &input.title)?;
    non_empty("symbol", &input.symbol)?;
    non_empty("timeframe", &input.timeframe)?;
    if input.time_axis.len() > MAX_TIME_AXIS {
        return Err(err(format!("time_axis exceeds {MAX_TIME_AXIS} entries")));
    }
    let obj = input
        .scene
        .as_object()
        .ok_or_else(|| err("scene must be an object"))?;
    let mut counts = serde_json::Map::new();
    for key in SCENE_ARRAYS {
        let n = obj
            .get(key)
            .and_then(Value::as_array)
            .map(Vec::len)
            .ok_or_else(|| err(format!("scene.{key} must be an array")))?;
        counts.insert(key.to_string(), json!(n));
    }
    let c = |k: &str| counts.get(k).and_then(Value::as_u64).unwrap_or(0);
    let summary = format!(
        "Scene '{}' for {} {}: {} signals, {} bands, {} stops, {} hints, {} hedges",
        input.title,
        input.symbol,
        input.timeframe,
        c("signals"),
        c("bands"),
        c("stops"),
        c("hints"),
        c("hedges"),
    );
    let mut items: Vec<MetricItem> = SCENE_ARRAYS
        .iter()
        .map(|k| MetricItem {
            label: (*k).to_string(),
            value: c(k).to_string(),
        })
        .collect();
    items.extend(input.kpis.iter().map(|(k, v)| MetricItem {
        label: k.clone(),
        value: v.clone(),
    }));
    let data = json!({
        "symbol": input.symbol,
        "timeframe": input.timeframe,
        "counts": Value::Object(counts),
        "kpis": input.kpis,
    });
    let mut env = TantraResultEnvelope::ok(summary, data)
        .with_block(DisplayBlock::MetricGrid { items })
        .with_block(DisplayBlock::OverlayScene {
            title: input.title.clone(),
            symbol: input.symbol.clone(),
            timeframe: input.timeframe.clone(),
            scene: input.scene.clone(),
            time_axis: input.time_axis.clone(),
        });
    if let Some(n) = input.notes.as_ref().filter(|n| !n.trim().is_empty()) {
        env = env.with_block(DisplayBlock::Alert {
            level: "info".to_string(),
            message: n.clone(),
        });
    }
    Ok(env)
}
