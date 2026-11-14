//! Strategy-manifest store port (plan 0007 Track 5 Phase 13, Jesse-MCP strategy CRUD).

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ports::error::PortError;

/// One strategy manifest (declarative spec, mirroring the catalog `step.toml`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrategySpec {
    /// Stable strategy identifier (e.g. "turtle_breakout").
    pub strategy_id: String,
    /// Human-readable name.
    pub name: String,
    /// Instrument universe the strategy trades, comma-separated canonical ids.
    pub instruments: Vec<String>,
    /// Declared parameter names (mirrors `Parameter(default, min, max, step)`).
    pub parameters: Vec<StrategyParameterSpec>,
    /// Created timestamp.
    pub created_at: DateTime<Utc>,
    /// Last-updated timestamp.
    pub updated_at: DateTime<Utc>,
}

/// One declarative strategy parameter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrategyParameterSpec {
    /// Parameter name.
    pub name: String,
    /// Default value (string-encoded exact decimal, mirroring the catalog).
    pub default: String,
    /// Inclusive minimum.
    pub min: String,
    /// Inclusive maximum.
    pub max: String,
    /// Step alignment.
    pub step: String,
}

/// Strategy-manifest CRUD port (plan 0007 Track 5 Phase 13).
///
/// Upstream state: the runtime does not wire a strategy store into the chat
/// agent yet — see `tantra-runtime/docs/gaps/0007` item 18. Implementors without
/// a store must answer `PortError::unavailable` citing that gap.
#[async_trait]
pub trait StrategyStore: Send + Sync + 'static {
    /// Save (create or update) a strategy manifest; returns the stored spec.
    async fn save_strategy(&self, spec: &StrategySpec) -> Result<StrategySpec, PortError>;

    /// List all stored strategies.
    async fn list_strategies(&self) -> Result<Vec<StrategySpec>, PortError>;

    /// Fetch one strategy by id.
    async fn get_strategy(&self, strategy_id: &str) -> Result<StrategySpec, PortError>;

    /// Delete one strategy by id; `false` when it did not exist.
    async fn delete_strategy(&self, strategy_id: &str) -> Result<bool, PortError>;
}
