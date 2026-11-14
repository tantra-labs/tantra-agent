//! Mutual-fund analytics chat tools (plan 0007 Track 3 Phase 7).

use std::sync::Arc;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;
use tantra_agent_chat::permission::{ToolRisk, ToolTraits};
use tantra_agent_chat::tool::{ChatTool, PlanContext, ToolContext, ToolPlan};
use tantra_agent_core::{ToolError, ToolOutput, ToolSpec};

use crate::ports::fund::FundReader;

/// Maximum number of schemes returned per search (token budget cap, §6.5).
pub const MAX_FUNDS_LIMIT: usize = 20;

/// Input arguments for `fund_search`.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FundSearchInput {
    /// Search query — scheme name or code fragment (e.g. "parag parikh", "15028").
    pub query: String,
    /// Maximum number of schemes to return (capped at 20).
    pub limit: Option<usize>,
}

/// Chat tool searching mutual-fund schemes (plan 0007 Track 3 Phase 7).
pub struct FundSearchTool {
    reader: Arc<dyn FundReader>,
    spec: ToolSpec,
}

impl FundSearchTool {
    /// Create new fund-search tool.
    pub fn new(reader: Arc<dyn FundReader>) -> Self {
        let schema = schemars::schema_for!(FundSearchInput);
        let spec = ToolSpec::new(
            "fund_search",
            "Search mutual-fund schemes by name or code, returning NAV, category, CAGR returns and expense ratio.",
            serde_json::to_value(schema).unwrap_or_default(),
        );
        Self { reader, spec }
    }
}

#[async_trait]
impl ChatTool for FundSearchTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn traits(&self) -> ToolTraits {
        ToolTraits::new(ToolRisk::ReadOnly)
    }

    fn plan(&self, input: &Value, _ctx: &PlanContext<'_>) -> Result<ToolPlan, ToolError> {
        let q = input.get("query").and_then(Value::as_str).unwrap_or("");
        Ok(ToolPlan::new(format!(
            "Search mutual-fund schemes for `{q}`"
        )))
    }

    async fn execute(&self, input: Value, _ctx: ToolContext) -> Result<ToolOutput, ToolError> {
        let args: FundSearchInput =
            serde_json::from_value(input).map_err(|e| ToolError::InvalidArguments {
                tool: "fund_search".to_string(),
                message: format!("schema validation failure: {e}"),
            })?;

        let capped = args.limit.unwrap_or(MAX_FUNDS_LIMIT).min(MAX_FUNDS_LIMIT);
        let funds = self
            .reader
            .fund_search(&args.query, Some(capped))
            .await
            .map_err(|e| e.into_tool_error("fund_search"))?;

        let funds_json: Vec<Value> = funds
            .into_iter()
            .take(MAX_FUNDS_LIMIT)
            .map(|f| {
                serde_json::json!({
                    "scheme_code": f.scheme_code,
                    "scheme_name": f.scheme_name,
                    "category": f.category,
                    "nav": f.nav.map(|m| m.amount),
                    "cagr_1y_pct": f.cagr_1y_pct,
                    "cagr_3y_pct": f.cagr_3y_pct,
                    "cagr_5y_pct": f.cagr_5y_pct,
                    "expense_ratio_pct": f.expense_ratio_pct,
                })
            })
            .collect();

        Ok(ToolOutput::ok(
            "fund_search",
            serde_json::json!({
                "funds": funds_json,
                "count": funds_json.len(),
            }),
        ))
    }
}

/// Input arguments for `fund_holdings`.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FundHoldingsInput {
    /// AMFI scheme code (e.g. "122639").
    pub scheme_code: String,
}

/// Chat tool fetching a mutual-fund scheme's portfolio holdings (plan 0007 Track 3 Phase 7).
pub struct FundHoldingsTool {
    reader: Arc<dyn FundReader>,
    spec: ToolSpec,
}

impl FundHoldingsTool {
    /// Create new fund-holdings tool.
    pub fn new(reader: Arc<dyn FundReader>) -> Self {
        let schema = schemars::schema_for!(FundHoldingsInput);
        let spec = ToolSpec::new(
            "fund_holdings",
            "Fetch the portfolio holdings (asset, weight, sector) of a mutual-fund scheme.",
            serde_json::to_value(schema).unwrap_or_default(),
        );
        Self { reader, spec }
    }
}

#[async_trait]
impl ChatTool for FundHoldingsTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn traits(&self) -> ToolTraits {
        ToolTraits::new(ToolRisk::ReadOnly)
    }

    fn plan(&self, input: &Value, _ctx: &PlanContext<'_>) -> Result<ToolPlan, ToolError> {
        let code = input
            .get("scheme_code")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        Ok(ToolPlan::new(format!(
            "Fetch portfolio holdings of scheme `{code}`"
        )))
    }

    async fn execute(&self, input: Value, _ctx: ToolContext) -> Result<ToolOutput, ToolError> {
        let args: FundHoldingsInput =
            serde_json::from_value(input).map_err(|e| ToolError::InvalidArguments {
                tool: "fund_holdings".to_string(),
                message: format!("schema validation failure: {e}"),
            })?;

        let holdings = self
            .reader
            .fund_holdings(&args.scheme_code)
            .await
            .map_err(|e| e.into_tool_error("fund_holdings"))?;

        let holdings_json: Vec<Value> = holdings
            .into_iter()
            .map(|h| {
                serde_json::json!({
                    "asset_name": h.asset_name,
                    "weight_pct": h.weight_pct,
                    "sector": h.sector,
                })
            })
            .collect();

        Ok(ToolOutput::ok(
            "fund_holdings",
            serde_json::json!({
                "scheme_code": args.scheme_code,
                "holdings": holdings_json,
                "count": holdings_json.len(),
            }),
        ))
    }
}
