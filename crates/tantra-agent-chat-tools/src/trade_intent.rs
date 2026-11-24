//! Safe trade-intent chat tool (plan 0007 Track 5 Phase 12, NoFX approval-queue pattern).

use std::sync::Arc;

use async_trait::async_trait;
use rust_decimal::Decimal;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;
use tantra_agent_chat::permission::{ToolRisk, ToolTraits};
use tantra_agent_chat::tool::{ChatTool, PlanContext, ToolContext, ToolPlan};
use tantra_agent_core::{ToolError, ToolOutput, ToolSpec};
use tantra_core::decision::Decision;
use tantra_core::order::OrderSide;
use tantra_core::{Price, Quantity};

use crate::market::parse_instrument;
use crate::ports::trade_intent::{TradeIntent, TradeIntentSink};

/// Validates a trade intent and formulates the closed `Decision` it encodes.
///
/// Returns the strict [`Decision::PlaceOrder`] the risk gate and execution
/// service consume. Market orders carry no `price_limit`; limit orders require
/// a positive price. Non-positive quantities are rejected.
pub fn trade_intent_to_decision(intent: &TradeIntent) -> Result<Decision, String> {
    if intent.quantity.value() <= Decimal::ZERO {
        return Err("quantity must be positive".to_string());
    }
    if intent.algo_id.trim().is_empty() {
        return Err("algo_id is required".to_string());
    }
    let side = match intent.side.as_str() {
        "buy" => OrderSide::Buy,
        "sell" => OrderSide::Sell,
        other => return Err(format!("side must be \"buy\" or \"sell\", got `{other}`")),
    };
    let price_limit = match intent.order_type.as_str() {
        "market" => None,
        "limit" => {
            let price = intent
                .price
                .ok_or_else(|| "limit orders require a price".to_string())?;
            if price.value() <= Decimal::ZERO {
                return Err("limit price must be positive".to_string());
            }
            Some(price)
        }
        other => {
            return Err(format!(
                "order_type must be \"limit\" or \"market\", got `{other}`"
            ));
        }
    };
    Ok(Decision::PlaceOrder {
        symbol: intent.instrument.symbol().to_string(),
        side,
        qty: intent.quantity,
        price_limit,
        algo_id: intent.algo_id.clone(),
        rationale: intent.rationale.clone(),
    })
}

/// Input arguments for `propose_trade`.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProposeTradeInput {
    /// Instrument symbol (e.g. "NSE:RELIANCE", "TCS").
    pub instrument: String,
    /// "buy" | "sell".
    pub side: String,
    /// "limit" | "market".
    pub order_type: String,
    /// Quantity as a decimal string (e.g. "100").
    pub quantity: String,
    /// Limit price as a decimal string (required for limit orders).
    pub price: Option<String>,
    /// SEBI algo identifier (4–32 chars of letters/digits/`_`/`-`).
    pub algo_id: String,
    /// True to execute in the paper-trading sandbox without the human gate.
    #[serde(default)]
    pub paper_trading: bool,
    /// Optional rationale from the model.
    pub rationale: Option<String>,
}

/// Chat tool submitting a validated trade intent into the approval queue or the
/// paper-trading sandbox (plan 0007 Track 5 Phase 12).
///
/// `ToolRisk::LocalWrite` — like the alert CRUD, it sits outside the all-ReadOnly
/// catalogue-count bundle.
pub struct ProposeTradeTool {
    sink: Arc<dyn TradeIntentSink>,
    spec: ToolSpec,
}

impl ProposeTradeTool {
    /// Create new propose-trade tool.
    pub fn new(sink: Arc<dyn TradeIntentSink>) -> Self {
        let schema = schemars::schema_for!(ProposeTradeInput);
        let spec = ToolSpec::new(
            "propose_trade",
            "Submit a validated trade intent that is held for approval (PendingApproval) or executed in the paper-trading sandbox. Never places a live order autonomously.",
            serde_json::to_value(schema).unwrap_or_default(),
        );
        Self { sink, spec }
    }
}

#[async_trait]
impl ChatTool for ProposeTradeTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn traits(&self) -> ToolTraits {
        ToolTraits::new(ToolRisk::LocalWrite)
    }

    fn plan(&self, input: &Value, _ctx: &PlanContext<'_>) -> Result<ToolPlan, ToolError> {
        let inst = input
            .get("instrument")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let paper = input
            .get("paper_trading")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let mode = if paper { "paper" } else { "approval" };
        Ok(ToolPlan::new(format!(
            "Submit a `{mode}` trade intent for `{inst}`"
        )))
    }

    async fn execute(&self, input: Value, _ctx: ToolContext) -> Result<ToolOutput, ToolError> {
        let args: ProposeTradeInput =
            serde_json::from_value(input).map_err(|e| ToolError::InvalidArguments {
                tool: "propose_trade".to_string(),
                message: format!("schema validation failure: {e}"),
            })?;

        let instrument = parse_instrument(&args.instrument)?;
        let quantity = Quantity::try_new(parse_decimal(&args.quantity).map_err(|e| {
            ToolError::InvalidArguments {
                tool: "propose_trade".to_string(),
                message: e,
            }
        })?)
        .map_err(|e| ToolError::InvalidArguments {
            tool: "propose_trade".to_string(),
            message: e.to_string(),
        })?;
        if quantity.value() <= Decimal::ZERO {
            return Err(ToolError::InvalidArguments {
                tool: "propose_trade".to_string(),
                message: "quantity must be positive".to_string(),
            });
        }
        let price = args
            .price
            .as_deref()
            .map(parse_decimal)
            .transpose()
            .map_err(|e| ToolError::InvalidArguments {
                tool: "propose_trade".to_string(),
                message: e.to_string(),
            })?
            .map(Price::try_new)
            .transpose()
            .map_err(|e| ToolError::InvalidArguments {
                tool: "propose_trade".to_string(),
                message: e.to_string(),
            })?;
        // SEBI algo-id format validation (mirrors `Decision::ALGO_ID_FORMAT`).
        let valid_algo = (4..=32).contains(&args.algo_id.len())
            && args
                .algo_id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
        if !valid_algo {
            return Err(ToolError::InvalidArguments {
                tool: "propose_trade".to_string(),
                message: format!(
                    "algo_id must match {}, got `{}`",
                    Decision::ALGO_ID_FORMAT,
                    args.algo_id
                ),
            });
        }

        let intent = TradeIntent {
            algo_id: args.algo_id,
            instrument,
            side: args.side,
            order_type: args.order_type,
            quantity,
            price,
            paper_trading: args.paper_trading,
            rationale: args.rationale,
        };
        let status = self
            .sink
            .propose_trade(&intent)
            .await
            .map_err(|e| e.into_tool_error("propose_trade"))?;

        Ok(ToolOutput::ok(
            "propose_trade",
            serde_json::json!({
                "intent_id": status.intent_id,
                "status": status.status,
                "message": status.message,
            }),
        ))
    }
}

/// Parses a decimal string (exact, never float).
pub(crate) fn parse_decimal(s: &str) -> Result<Decimal, String> {
    s.trim()
        .parse::<Decimal>()
        .map_err(|_| format!("`{s}` is not a valid decimal"))
}
