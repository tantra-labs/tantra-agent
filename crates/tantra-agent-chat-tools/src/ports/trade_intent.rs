//! Safe trade-intent bridge port (plan 0007 Track 5 Phase 12, NoFX approval-queue pattern).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tantra_core::{InstrumentId, Money, Price, Quantity};

use crate::ports::error::PortError;

/// A validated, pre-trade intent to place an order.
///
/// Formulated by the (untrusted) agent, strictly validated, then handed to a
/// `TradeIntentSink` that enqueues it into the runtime's approval queue
/// (`PendingApproval`) or executes it directly in the paper-trading sandbox
/// when `paper_trading` is set — never an autonomous live dispatch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TradeIntent {
    /// SEBI algo identifier for the audit trail (4–32 chars of `[A-Za-z0-9_-]`).
    pub algo_id: String,
    /// Instrument to trade.
    pub instrument: InstrumentId,
    /// "buy" | "sell".
    pub side: String,
    /// "limit" | "market".
    pub order_type: String,
    /// Desired quantity (must be positive).
    pub quantity: Quantity,
    /// Limit price, required for limit orders (must be positive).
    pub price: Option<Price>,
    /// True to execute in the paper-trading sandbox without the human gate.
    pub paper_trading: bool,
    /// Optional rationale from the model (audit trail).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
}

/// Outcome of submitting a trade intent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TradeIntentStatus {
    /// Stable intent/job identifier.
    pub intent_id: String,
    /// "pending_approval" | "paper_filled" | "rejected".
    pub status: String,
    /// Human-readable confirmation or refusal message.
    pub message: String,
}

/// Safe trade-intent sink port (plan 0007 Track 5 Phase 12).
///
/// Implementors submit validated intents to the runtime approval queue
/// (`FileJobRegistry` as `JobStatus::PendingApproval`) or the paper-trading
/// sandbox. Upstream state: the runtime does not wire a sink into the chat
/// agent yet — see `tantra-runtime/docs/gaps/0007` item 17. Implementors without
/// a sink must answer `PortError::unavailable` citing that gap.
#[async_trait]
pub trait TradeIntentSink: Send + Sync + 'static {
    /// Submit a validated trade intent for approval or paper execution.
    async fn propose_trade(&self, intent: &TradeIntent) -> Result<TradeIntentStatus, PortError>;
}

/// Estimated notional of a limit intent, used for display/validation.
pub fn intent_notional(intent: &TradeIntent) -> Option<Money> {
    let price = match (intent.order_type.as_str(), intent.price) {
        ("limit", Some(p)) => p,
        _ => return None,
    };
    Some(Money::inr(price.0 * intent.quantity.0))
}
