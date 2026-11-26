//! IPO calendar & subscription chat tools (plan 0007 Track 3 Phase 8).

use std::sync::Arc;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;
use tantra_agent_chat::permission::{ToolRisk, ToolTraits};
use tantra_agent_chat::tool::{ChatTool, PlanContext, ToolContext, ToolPlan};
use tantra_agent_core::{ToolError, ToolOutput, ToolSpec};

use crate::ports::ipo::IpoReader;

/// Maximum number of IPO issues returned per call (token budget cap, §6.5).
pub const MAX_IPO_LIMIT: usize = 20;

/// Input arguments for `ipo_calendar`.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IpoCalendarInput {
    /// Maximum number of issues to return (capped at 20).
    pub limit: Option<usize>,
}

/// Chat tool listing upcoming IPO issues (plan 0007 Track 3 Phase 8).
pub struct IpoCalendarTool {
    reader: Arc<dyn IpoReader>,
    spec: ToolSpec,
}

impl IpoCalendarTool {
    /// Create new IPO-calendar tool.
    pub fn new(reader: Arc<dyn IpoReader>) -> Self {
        let schema = schemars::schema_for!(IpoCalendarInput);
        let spec = ToolSpec::new(
            "ipo_calendar",
            "List upcoming IPO issues (open/close dates, price band, lot size, issue size) on NSE/BSE.",
            serde_json::to_value(schema).unwrap_or_default(),
        );
        Self { reader, spec }
    }
}

#[async_trait]
impl ChatTool for IpoCalendarTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn traits(&self) -> ToolTraits {
        ToolTraits::new(ToolRisk::ReadOnly)
    }

    fn plan(&self, _input: &Value, _ctx: &PlanContext<'_>) -> Result<ToolPlan, ToolError> {
        Ok(ToolPlan::new("List upcoming IPO issues".to_string()))
    }

    async fn execute(&self, input: Value, _ctx: ToolContext) -> Result<ToolOutput, ToolError> {
        let args: IpoCalendarInput =
            serde_json::from_value(input).map_err(|e| ToolError::InvalidArguments {
                tool: "ipo_calendar".to_string(),
                message: format!("schema validation failure: {e}"),
            })?;

        let capped = args.limit.unwrap_or(MAX_IPO_LIMIT).min(MAX_IPO_LIMIT);
        let issues = self
            .reader
            .upcoming_ipos(Some(capped))
            .await
            .map_err(|e| e.into_tool_error("ipo_calendar"))?;

        let issues_json: Vec<Value> = issues
            .into_iter()
            .take(MAX_IPO_LIMIT)
            .map(|i| {
                serde_json::json!({
                    "symbol": i.symbol,
                    "company_name": i.company_name,
                    "open_date": i.open_date,
                    "close_date": i.close_date,
                    "price_min": i.price_min.map(|p| p.value()),
                    "price_max": i.price_max.map(|p| p.value()),
                    "lot_size": i.lot_size,
                    "issue_size": i.issue_size.map(|m| m.amount()),
                })
            })
            .collect();

        Ok(ToolOutput::ok(
            "ipo_calendar",
            serde_json::json!({
                "issues": issues_json,
                "count": issues_json.len(),
            }),
        ))
    }
}

/// Input arguments for `ipo_subscription`.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IpoSubscriptionInput {
    /// Issue symbol (e.g. "ZAGGLECOM").
    pub symbol: String,
}

/// Chat tool fetching an IPO's live subscription status (plan 0007 Track 3 Phase 8).
pub struct IpoSubscriptionTool {
    reader: Arc<dyn IpoReader>,
    spec: ToolSpec,
}

impl IpoSubscriptionTool {
    /// Create new IPO-subscription tool.
    pub fn new(reader: Arc<dyn IpoReader>) -> Self {
        let schema = schemars::schema_for!(IpoSubscriptionInput);
        let spec = ToolSpec::new(
            "ipo_subscription",
            "Fetch the live subscription multiples (QIB/NII/RII/total) of an IPO issue.",
            serde_json::to_value(schema).unwrap_or_default(),
        );
        Self { reader, spec }
    }
}

#[async_trait]
impl ChatTool for IpoSubscriptionTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn traits(&self) -> ToolTraits {
        ToolTraits::new(ToolRisk::ReadOnly)
    }

    fn plan(&self, input: &Value, _ctx: &PlanContext<'_>) -> Result<ToolPlan, ToolError> {
        let symbol = input
            .get("symbol")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        Ok(ToolPlan::new(format!(
            "Fetch IPO subscription status for `{symbol}`"
        )))
    }

    async fn execute(&self, input: Value, _ctx: ToolContext) -> Result<ToolOutput, ToolError> {
        let args: IpoSubscriptionInput =
            serde_json::from_value(input).map_err(|e| ToolError::InvalidArguments {
                tool: "ipo_subscription".to_string(),
                message: format!("schema validation failure: {e}"),
            })?;

        let subscription = self
            .reader
            .ipo_subscription(&args.symbol)
            .await
            .map_err(|e| e.into_tool_error("ipo_subscription"))?;

        Ok(ToolOutput::ok(
            "ipo_subscription",
            serde_json::json!({
                "symbol": subscription.symbol,
                "as_of": subscription.as_of,
                "qib_multiple": subscription.qib_multiple,
                "nii_multiple": subscription.nii_multiple,
                "rii_multiple": subscription.rii_multiple,
                "total_multiple": subscription.total_multiple,
            }),
        ))
    }
}
