//! Fundamental valuation & financial-statement chat tools (plan 0007 Track 3 Phase 6).

use std::sync::Arc;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;
use tantra_agent_chat::permission::{ToolRisk, ToolTraits};
use tantra_agent_chat::tool::{ChatTool, PlanContext, ToolContext, ToolPlan};
use tantra_agent_core::{ToolError, ToolOutput, ToolSpec};

use crate::market::parse_instrument;
use crate::ports::fundamental::FundamentalReader;

/// Maximum number of financial statements returned per call (token budget cap, §6.5).
pub const MAX_STATEMENTS_LIMIT: usize = 20;

/// Input arguments for `fundamental_ratios`.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FundamentalsInput {
    /// Instrument symbol (e.g. "NSE:RELIANCE", "TCS").
    pub instrument: String,
}

/// Chat tool fetching fundamental valuation ratios (plan 0007 Track 3 Phase 6).
pub struct FundamentalsTool {
    reader: Arc<dyn FundamentalReader>,
    spec: ToolSpec,
}

impl FundamentalsTool {
    /// Create new fundamentals tool.
    pub fn new(reader: Arc<dyn FundamentalReader>) -> Self {
        let schema = schemars::schema_for!(FundamentalsInput);
        let spec = ToolSpec::new(
            "fundamental_ratios",
            "Fetch fundamental valuation ratios (P/E, P/B, EV/EBITDA, dividend yield, ROCE, ROE, debt/equity, interest coverage) for an instrument.",
            serde_json::to_value(schema).unwrap_or_default(),
        );
        Self { reader, spec }
    }
}

#[async_trait]
impl ChatTool for FundamentalsTool {
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
            .unwrap_or("unknown");
        Ok(ToolPlan::new(format!(
            "Fetch fundamental ratios for `{inst}`"
        )))
    }

    async fn execute(&self, input: Value, _ctx: ToolContext) -> Result<ToolOutput, ToolError> {
        let args: FundamentalsInput =
            serde_json::from_value(input).map_err(|e| ToolError::InvalidArguments {
                tool: "fundamental_ratios".to_string(),
                message: format!("schema validation failure: {e}"),
            })?;

        let inst = parse_instrument(&args.instrument)?;
        let ratios = self
            .reader
            .fundamental_ratios(&inst)
            .await
            .map_err(|e| e.into_tool_error("fundamental_ratios"))?;

        Ok(ToolOutput::ok(
            "fundamental_ratios",
            serde_json::json!({
                "instrument": ratios.instrument.to_string(),
                "as_of": ratios.as_of,
                "pe_ratio": ratios.pe_ratio,
                "pb_ratio": ratios.pb_ratio,
                "ev_ebitda": ratios.ev_ebitda,
                "dividend_yield_pct": ratios.dividend_yield_pct,
                "roce_pct": ratios.roce_pct,
                "roe_pct": ratios.roe_pct,
                "debt_equity": ratios.debt_equity,
                "interest_coverage": ratios.interest_coverage,
            }),
        ))
    }
}

/// Input arguments for `financial_statements`.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FinancialStatementsInput {
    /// Instrument symbol (e.g. "NSE:RELIANCE", "TCS").
    pub instrument: String,
    /// Statement type filter — one of "income_statement", "balance_sheet", "cash_flow".
    pub statement_type: Option<String>,
    /// Maximum number of statements to return (capped at 20).
    pub limit: Option<usize>,
}

/// Chat tool fetching financial statements (plan 0007 Track 3 Phase 6).
pub struct FinancialStatementsTool {
    reader: Arc<dyn FundamentalReader>,
    spec: ToolSpec,
}

impl FinancialStatementsTool {
    /// Create new financial-statements tool.
    pub fn new(reader: Arc<dyn FundamentalReader>) -> Self {
        let schema = schemars::schema_for!(FinancialStatementsInput);
        let spec = ToolSpec::new(
            "financial_statements",
            "Fetch recent financial statements (income statement, balance sheet, cash flow) for an instrument.",
            serde_json::to_value(schema).unwrap_or_default(),
        );
        Self { reader, spec }
    }
}

#[async_trait]
impl ChatTool for FinancialStatementsTool {
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
            .unwrap_or("unknown");
        Ok(ToolPlan::new(format!(
            "Fetch financial statements for `{inst}`"
        )))
    }

    async fn execute(&self, input: Value, _ctx: ToolContext) -> Result<ToolOutput, ToolError> {
        let args: FinancialStatementsInput =
            serde_json::from_value(input).map_err(|e| ToolError::InvalidArguments {
                tool: "financial_statements".to_string(),
                message: format!("schema validation failure: {e}"),
            })?;

        let inst = parse_instrument(&args.instrument)?;
        let capped = args
            .limit
            .unwrap_or(MAX_STATEMENTS_LIMIT)
            .min(MAX_STATEMENTS_LIMIT);
        let statements = self
            .reader
            .financial_statements(&inst, args.statement_type.as_deref(), Some(capped))
            .await
            .map_err(|e| e.into_tool_error("financial_statements"))?;

        let statements_json: Vec<Value> = statements
            .into_iter()
            .take(MAX_STATEMENTS_LIMIT)
            .map(|s| {
                serde_json::json!({
                    "instrument": s.instrument.to_string(),
                    "statement_type": s.statement_type,
                    "fiscal_quarter": s.fiscal_quarter,
                    "revenue": s.revenue.map(|m| m.amount),
                    "operating_profit": s.operating_profit.map(|m| m.amount),
                    "net_income": s.net_income.map(|m| m.amount),
                    "eps": s.eps,
                })
            })
            .collect();

        Ok(ToolOutput::ok(
            "financial_statements",
            serde_json::json!({
                "statements": statements_json,
                "count": statements_json.len(),
            }),
        ))
    }
}
