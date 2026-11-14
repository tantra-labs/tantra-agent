//! Read-only fundamentals / financial-statements port (plan 0007 Track 3 Phase 6).

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tantra_core::{InstrumentId, Money};

use crate::ports::error::PortError;

/// Fundamental valuation ratios for one instrument.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FundamentalRatios {
    /// Instrument the ratios belong to.
    pub instrument: InstrumentId,
    /// Snapshot timestamp.
    pub as_of: DateTime<Utc>,
    /// Price-to-earnings ratio.
    pub pe_ratio: Option<Decimal>,
    /// Price-to-book ratio.
    pub pb_ratio: Option<Decimal>,
    /// Enterprise value / EBITDA.
    pub ev_ebitda: Option<Decimal>,
    /// Dividend yield as a percent.
    pub dividend_yield_pct: Option<Decimal>,
    /// Return on capital employed as a percent.
    pub roce_pct: Option<Decimal>,
    /// Return on equity as a percent.
    pub roe_pct: Option<Decimal>,
    /// Debt-to-equity ratio.
    pub debt_equity: Option<Decimal>,
    /// Interest coverage ratio (EBIT / interest expense).
    pub interest_coverage: Option<Decimal>,
}

/// One period's financial statement (income statement / balance sheet / cash flow).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FinancialStatement {
    /// Instrument the statement belongs to.
    pub instrument: InstrumentId,
    /// Statement type: "income_statement" | "balance_sheet" | "cash_flow".
    pub statement_type: String,
    /// Fiscal period label, e.g. "FY2026-Q1".
    pub fiscal_quarter: String,
    /// Revenue for the period.
    pub revenue: Option<Money>,
    /// Operating profit for the period.
    pub operating_profit: Option<Money>,
    /// Net income for the period.
    pub net_income: Option<Money>,
    /// Earnings per share for the period.
    pub eps: Option<Decimal>,
}

/// Read-only fundamentals port (plan 0007 Track 3 Phase 6).
///
/// Upstream state: fundamentals/financial-statement data does not exist anywhere yet —
/// see `tantra-data/docs/gaps/0020-fundamentals-ingestion.md`. Implementors without a
/// source must answer `PortError::unavailable` citing that gap — never fabricated ratios.
#[async_trait]
pub trait FundamentalReader: Send + Sync + 'static {
    /// Fetch the latest fundamental valuation ratios for an instrument.
    async fn fundamental_ratios(
        &self,
        instrument: &InstrumentId,
    ) -> Result<FundamentalRatios, PortError>;

    /// Fetch recent financial statements, optionally filtered by statement type,
    /// capped at `limit`.
    async fn financial_statements(
        &self,
        instrument: &InstrumentId,
        statement_type: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<FinancialStatement>, PortError>;
}
