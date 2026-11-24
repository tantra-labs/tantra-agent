//! Capital-gains tax reporting chat tool (plan 0007 Track 5 Phase 14).

use std::sync::Arc;

use async_trait::async_trait;
use rust_decimal::Decimal;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;
use tantra_agent_chat::permission::{ToolRisk, ToolTraits};
use tantra_agent_chat::tool::{ChatTool, PlanContext, ToolContext, ToolPlan};
use tantra_agent_core::{ToolError, ToolOutput, ToolSpec};
use tantra_core::{AccountId, Currency, Money};

use crate::ports::portfolio::DateRange;
use crate::ports::tax::{ClosedLotRecord, TaxReportReader};

/// Indian equity capital-gains tax rates (Finance Act 2025 schedule):
/// STCG 20% flat; LTCG 12.5% on the gain above the ₹1.25L exemption.
pub const STCG_RATE: Decimal = Decimal::from_parts(20, 0, 0, false, 2); // 0.20
pub const LTCG_RATE: Decimal = Decimal::from_parts(125, 0, 0, false, 3); // 0.125
/// Long-term exemption threshold in INR (₹1,25,000).
pub const LTCG_EXEMPTION: Decimal = Decimal::from_parts(1250000, 0, 0, false, 0);

/// The computed capital-gains tax summary for a set of closed lots.
#[derive(Debug, Clone, PartialEq)]
pub struct TaxSummary {
    /// Total short-term capital gain (losses reduce the base).
    pub stcg_pnl: Money,
    /// Total long-term capital gain.
    pub ltcg_pnl: Money,
    /// STCG tax = 20% of positive STCG.
    pub stcg_tax: Money,
    /// LTCG above the exemption threshold.
    pub ltcg_taxable: Money,
    /// LTCG tax = 12.5% of taxable LTCG.
    pub ltcg_tax: Money,
    /// STCG + LTCG tax.
    pub total_tax: Money,
}

/// Computes the Indian capital-gains tax from closed lots, all exact `Decimal`.
///
/// STCG is taxed at [`STCG_RATE`] when positive; LTCG is taxed at [`LTCG_RATE`]
/// only on the portion above [`LTCG_EXEMPTION`]. Currency is taken from the
/// first lot's realized P&L (INR by default).
pub fn compute_capital_gains(lots: &[ClosedLotRecord]) -> TaxSummary {
    let currency = lots
        .first()
        .map(|lot| lot.realized_pnl.currency)
        .unwrap_or(Currency::Inr);
    let mut stcg_pnl = Decimal::ZERO;
    let mut ltcg_pnl = Decimal::ZERO;
    for lot in lots {
        let pnl = lot.realized_pnl.amount;
        if lot.is_long_term {
            ltcg_pnl += pnl;
        } else {
            stcg_pnl += pnl;
        }
    }
    let stcg_tax = if stcg_pnl > Decimal::ZERO {
        (stcg_pnl * STCG_RATE).round_dp(2)
    } else {
        Decimal::ZERO
    };
    let ltcg_taxable = if ltcg_pnl > LTCG_EXEMPTION {
        ltcg_pnl - LTCG_EXEMPTION
    } else {
        Decimal::ZERO
    };
    let ltcg_tax = (ltcg_taxable * LTCG_RATE).round_dp(2);
    TaxSummary {
        stcg_pnl: Money::new(stcg_pnl, currency),
        ltcg_pnl: Money::new(ltcg_pnl, currency),
        stcg_tax: Money::new(stcg_tax, currency),
        ltcg_taxable: Money::new(ltcg_taxable, currency),
        ltcg_tax: Money::new(ltcg_tax, currency),
        total_tax: Money::new(stcg_tax + ltcg_tax, currency),
    }
}

/// Input arguments for `tax_report`.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaxReportInput {
    /// Account identifier (e.g. "ACC-01").
    pub account: String,
    /// Inclusive start of the realized range (RFC3339).
    pub start: Option<String>,
    /// Exclusive end of the realized range (RFC3339).
    pub end: Option<String>,
}

/// Chat tool computing Indian capital-gains tax (STCG/LTCG) from closed tax
/// lots (plan 0007 Track 5 Phase 14).
pub struct TaxReportTool {
    reader: Arc<dyn TaxReportReader>,
    spec: ToolSpec,
}

impl TaxReportTool {
    /// Create new tax-report tool.
    pub fn new(reader: Arc<dyn TaxReportReader>) -> Self {
        let schema = schemars::schema_for!(TaxReportInput);
        let spec = ToolSpec::new(
            "tax_report",
            "Compute Indian capital-gains tax (STCG 20% flat, LTCG 12.5% over the 1.25L exemption) from closed tax lots.",
            serde_json::to_value(schema).unwrap_or_default(),
        );
        Self { reader, spec }
    }
}

#[async_trait]
impl ChatTool for TaxReportTool {
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
            "Compute capital-gains tax for account `{account}`"
        )))
    }

    async fn execute(&self, input: Value, _ctx: ToolContext) -> Result<ToolOutput, ToolError> {
        let args: TaxReportInput =
            serde_json::from_value(input).map_err(|e| ToolError::InvalidArguments {
                tool: "tax_report".to_string(),
                message: format!("schema validation failure: {e}"),
            })?;

        let account = AccountId::new(&args.account);
        let start = args.start.as_deref().and_then(parse_ts);
        let end = args.end.as_deref().and_then(parse_ts);
        let lots = self
            .reader
            .closed_lots(&account, DateRange::new(start, end))
            .await
            .map_err(|e| e.into_tool_error("tax_report"))?;

        let summary = compute_capital_gains(&lots);
        let lots_json: Vec<Value> = lots
            .into_iter()
            .map(|lot| {
                serde_json::json!({
                    "lot_id": lot.lot_id,
                    "instrument": lot.instrument.to_string(),
                    "quantity": lot.quantity.value(),
                    "entry_price": lot.entry_price.value(),
                    "exit_price": lot.exit_price.value(),
                    "realized_pnl": lot.realized_pnl.amount,
                    "holding_period_days": lot.holding_period_days,
                    "is_long_term": lot.is_long_term,
                })
            })
            .collect();

        Ok(ToolOutput::ok(
            "tax_report",
            serde_json::json!({
                "account": args.account,
                "summary": {
                    "stcg_pnl": summary.stcg_pnl.amount,
                    "ltcg_pnl": summary.ltcg_pnl.amount,
                    "stcg_tax": summary.stcg_tax.amount,
                    "ltcg_taxable": summary.ltcg_taxable.amount,
                    "ltcg_tax": summary.ltcg_tax.amount,
                    "total_tax": summary.total_tax.amount,
                },
                "lots": lots_json,
                "count": lots_json.len(),
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
