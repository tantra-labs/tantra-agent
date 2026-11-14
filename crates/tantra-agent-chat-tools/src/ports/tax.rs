//! Tax / ledger-statement read port (plan 0007 Track 5 Phase 14).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tantra_core::{AccountId, InstrumentId, Money, Price, Quantity};

use crate::ports::error::PortError;
use crate::ports::portfolio::DateRange;

/// One fully closed (matched-away) tax lot with its realized result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClosedLotRecord {
    /// Tax-lot identifier.
    pub lot_id: String,
    /// Instrument the lot was traded in.
    pub instrument: InstrumentId,
    /// Quantity matched away.
    pub quantity: Quantity,
    /// Entry (cost-basis) price per unit.
    pub entry_price: Price,
    /// Exit price per unit.
    pub exit_price: Price,
    /// Realized P&L of the matched quantity.
    pub realized_pnl: Money,
    /// Holding period in days.
    pub holding_period_days: i64,
    /// True when the holding qualifies as long-term (equity: > 12 months).
    pub is_long_term: bool,
}

/// Read-only tax/statement port (plan 0007 Track 5 Phase 14).
///
/// Upstream state: `tantra-ledger-portfolio` already computes matched lots
/// (`MatchedLot`) but no runtime bridge exposes them to the chat agent yet —
/// see `tantra-runtime/docs/gaps/0007` item 19. Implementors without a bridge
/// must answer `PortError::unavailable` citing that gap.
#[async_trait]
pub trait TaxReportReader: Send + Sync + 'static {
    /// Fetch fully closed tax lots for an account over a date range.
    async fn closed_lots(
        &self,
        account: &AccountId,
        range: DateRange,
    ) -> Result<Vec<ClosedLotRecord>, PortError>;
}
