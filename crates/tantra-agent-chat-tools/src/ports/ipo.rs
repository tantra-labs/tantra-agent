//! Read-only IPO calendar / subscription port (plan 0007 Track 3 Phase 8).

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tantra_core::{Money, Price};

use crate::ports::error::PortError;

/// One upcoming IPO issue.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IpoIssue {
    /// Issue symbol (e.g. "ZAGGLECOM").
    pub symbol: String,
    /// Company name.
    pub company_name: String,
    /// Subscription open date.
    pub open_date: DateTime<Utc>,
    /// Subscription close date.
    pub close_date: DateTime<Utc>,
    /// Lower end of the price band.
    pub price_min: Option<Price>,
    /// Upper end of the price band.
    pub price_max: Option<Price>,
    /// Minimum application lot size in shares.
    pub lot_size: Option<u64>,
    /// Total issue size (INR).
    pub issue_size: Option<Money>,
}

/// One IPO's live subscription status.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IpoSubscription {
    /// Issue symbol.
    pub symbol: String,
    /// Snapshot timestamp.
    pub as_of: DateTime<Utc>,
    /// QIB subscription multiple.
    pub qib_multiple: Option<Decimal>,
    /// NII subscription multiple.
    pub nii_multiple: Option<Decimal>,
    /// RII subscription multiple.
    pub rii_multiple: Option<Decimal>,
    /// Total subscription multiple.
    pub total_multiple: Option<Decimal>,
}

/// Read-only IPO tracking port (plan 0007 Track 3 Phase 8).
///
/// Upstream state: no IPO calendar/subscription data source exists anywhere yet — see
/// `tantra-data/docs/gaps/0022-ipo-calendar-and-subscription-ingestion.md`. Implementors
/// without a source must answer `PortError::unavailable` citing that gap — never
/// fabricated issues.
#[async_trait]
pub trait IpoReader: Send + Sync + 'static {
    /// Fetch upcoming (open or scheduled) IPO issues, capped at `limit`.
    async fn upcoming_ipos(&self, limit: Option<usize>) -> Result<Vec<IpoIssue>, PortError>;

    /// Fetch the live subscription status of one issue.
    async fn ipo_subscription(&self, symbol: &str) -> Result<IpoSubscription, PortError>;
}
