//! Read-only mutual-fund analytics port (plan 0007 Track 3 Phase 7).

use async_trait::async_trait;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tantra_core::Money;

use crate::ports::error::PortError;

/// One mutual-fund scheme's metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MutualFundMeta {
    /// AMFI scheme code.
    pub scheme_code: String,
    /// Scheme name.
    pub scheme_name: String,
    /// Category (e.g. "Large Cap", "Mid Cap", "Flexi Cap", "Liquid").
    pub category: String,
    /// Latest net-asset-value per unit.
    pub nav: Option<Money>,
    /// 1-year CAGR as a percent.
    pub cagr_1y_pct: Option<Decimal>,
    /// 3-year CAGR as a percent.
    pub cagr_3y_pct: Option<Decimal>,
    /// 5-year CAGR as a percent.
    pub cagr_5y_pct: Option<Decimal>,
    /// Expense ratio as a percent.
    pub expense_ratio_pct: Option<Decimal>,
}

/// One holding of a mutual-fund scheme.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FundHolding {
    /// Asset / security name.
    pub asset_name: String,
    /// Portfolio weight as a percent.
    pub weight_pct: Decimal,
    /// Sector the holding belongs to, when classified.
    pub sector: Option<String>,
}

/// Read-only mutual-fund analytics port (plan 0007 Track 3 Phase 7).
///
/// Upstream state: no mutual-fund NAV/holdings data source exists anywhere yet — see
/// `tantra-data/docs/gaps/0021-mutual-fund-data-ingestion.md`. Implementors without a
/// source must answer `PortError::unavailable` citing that gap — never fabricated funds.
#[async_trait]
pub trait FundReader: Send + Sync + 'static {
    /// Search schemes by query string (name or code fragment), capped at `limit`.
    async fn fund_search(
        &self,
        query: &str,
        limit: Option<usize>,
    ) -> Result<Vec<MutualFundMeta>, PortError>;

    /// Fetch the portfolio holdings of one scheme.
    async fn fund_holdings(&self, scheme_code: &str) -> Result<Vec<FundHolding>, PortError>;
}
