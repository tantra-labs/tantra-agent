//! Trade ledger statement chat tool (plan 0007 Track 5 Phase 14).

use std::sync::Arc;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;
use tantra_agent_chat::permission::{ToolRisk, ToolTraits};
use tantra_agent_chat::tool::{ChatTool, PlanContext, ToolContext, ToolPlan};
use tantra_agent_core::{ToolError, ToolOutput, ToolSpec};
use tantra_core::AccountId;

use crate::ports::portfolio::DateRange;
use crate::ports::tax::{ClosedLotRecord, TaxReportReader};
use crate::tax::compute_capital_gains;

/// Input arguments for `ledger_statement`.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LedgerStatementInput {
    /// Account identifier (e.g. "ACC-01").
    pub account: String,
    /// Inclusive start of the statement period (RFC3339).
    pub start: Option<String>,
    /// Exclusive end of the statement period (RFC3339).
    pub end: Option<String>,
}

/// Formats closed tax lots as a markdown statement table plus the tax summary.
///
/// Pure formatting over already-fetched lots — exact `Decimal`, never a float.
pub fn format_ledger_statement(lots: &[ClosedLotRecord]) -> String {
    let summary = compute_capital_gains(lots);
    let mut out = String::from("| Lot | Instrument | Qty | Entry | Exit | P&L | Days | LT |\n");
    out.push_str("|---|---|---|---|---|---|---|---:|\n");
    for lot in lots {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} |\n",
            lot.lot_id,
            lot.instrument,
            lot.quantity.value(),
            lot.entry_price.value(),
            lot.exit_price.value(),
            lot.realized_pnl.amount(),
            lot.holding_period_days,
            if lot.is_long_term { "LT" } else { "ST" }
        ));
    }
    out.push_str(&format!(
        "\n**Summary** — STCG: {} | LTCG: {} | STCG tax (20%): {} | LTCG taxable: {} | LTCG tax (12.5%): {} | **Total tax: {}**\n",
        summary.stcg_pnl.amount(),
        summary.ltcg_pnl.amount(),
        summary.stcg_tax.amount(),
        summary.ltcg_taxable.amount(),
        summary.ltcg_tax.amount(),
        summary.total_tax.amount()
    ));
    out
}

/// Chat tool rendering a trade ledger statement (markdown) from closed tax lots
/// (plan 0007 Track 5 Phase 14).
pub struct LedgerStatementTool {
    reader: Arc<dyn TaxReportReader>,
    spec: ToolSpec,
}

impl LedgerStatementTool {
    /// Create new ledger-statement tool.
    pub fn new(reader: Arc<dyn TaxReportReader>) -> Self {
        let schema = schemars::schema_for!(LedgerStatementInput);
        let spec = ToolSpec::new(
            "ledger_statement",
            "Render a trade ledger statement (closed lots + realized P&L + capital-gains tax summary) as markdown.",
            serde_json::to_value(schema).unwrap_or_default(),
        );
        Self { reader, spec }
    }
}

#[async_trait]
impl ChatTool for LedgerStatementTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn traits(&self) -> ToolTraits {
        ToolTraits::new(ToolRisk::ReadOnly)
    }

    fn plan(&self, input: &Value, _ctx: &PlanContext<'_>) -> Result<ToolPlan, ToolError> {
        let account = input
            .get("account")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        Ok(ToolPlan::new(format!(
            "Render ledger statement for account `{account}`"
        )))
    }

    async fn execute(&self, input: Value, _ctx: ToolContext) -> Result<ToolOutput, ToolError> {
        let args: LedgerStatementInput =
            serde_json::from_value(input).map_err(|e| ToolError::InvalidArguments {
                tool: "ledger_statement".to_string(),
                message: format!("schema validation failure: {e}"),
            })?;

        let account = AccountId::new(&args.account);
        let start = args.start.as_deref().and_then(parse_ts);
        let end = args.end.as_deref().and_then(parse_ts);
        let lots = self
            .reader
            .closed_lots(&account, DateRange::new(start, end))
            .await
            .map_err(|e| e.into_tool_error("ledger_statement"))?;

        let statement = format_ledger_statement(&lots);

        Ok(ToolOutput::ok(
            "ledger_statement",
            serde_json::json!({
                "account": args.account,
                "statement": statement,
                "count": lots.len(),
            }),
        ))
    }
}

/// Local RFC3339 parser (mirrors `crate::calendar::parse_ts`).
fn parse_ts(s: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.to_utc())
}
