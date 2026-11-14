//! Read-only macro/economic calendar port (plan 0007 Track 2 Phase 5).

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ports::error::PortError;

/// One scheduled economic event (macro calendar).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EconomicEvent {
    /// Stable event identifier.
    pub id: String,
    /// ISO country code (e.g. "IN", "US").
    pub country: String,
    /// Event title (e.g. "RBI Repo Rate Decision").
    pub title: String,
    /// Reporting currency, when known.
    pub currency: Option<String>,
    /// "high" | "medium" | "low".
    pub impact: String,
    /// Market-consensus forecast, when published.
    pub forecast: Option<String>,
    /// Prior release value, when known.
    pub previous: Option<String>,
    /// Actual released value, when published.
    pub actual: Option<String>,
    /// Scheduled release instant.
    pub event_date: DateTime<Utc>,
}

/// Filter for economic-calendar queries.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EconomicCalendarQuery {
    /// ISO country code filter (e.g. "IN", "US").
    pub country: Option<String>,
    /// Impact filter: "high" | "medium" | "low".
    pub impact: Option<String>,
    /// Inclusive start of the event window.
    pub start: Option<DateTime<Utc>>,
    /// Exclusive end of the event window.
    pub end: Option<DateTime<Utc>>,
    /// Maximum number of events to return.
    pub limit: Option<usize>,
}

/// Read-only macro/economic calendar port (plan 0007 Track 2 Phase 5).
///
/// Implemented by runtime application layers over an economic-calendar source. Upstream
/// state: the calendar exists Python-side but has no Rust-consumer bridge —
/// `tantra-data/docs/gaps/0018-economic-calendar-rust-bridge.md`. Implementors without a
/// bridge must answer `PortError::unavailable` citing that gap — never an empty success.
#[async_trait]
pub trait CalendarReader: Send + Sync + 'static {
    /// Fetch scheduled economic events matching the query.
    async fn economic_calendar(
        &self,
        query: &EconomicCalendarQuery,
    ) -> Result<Vec<EconomicEvent>, PortError>;
}
