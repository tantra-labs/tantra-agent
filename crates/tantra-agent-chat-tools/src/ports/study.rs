//! Read-only experimental statistical study port (plan 0016).
//!
//! Decouples `tantra-agent-chat-tools` from `tantra-market-finance` and `tantra-runtime`
//! (`Rule R1/R2/R4`) while letting the runtime wire exact-`Decimal` study summaries
//! into conversational tools.

use async_trait::async_trait;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tantra_core::InstrumentId;

use crate::ports::error::PortError;

/// Probability exceedance level row returned inside [`StudySummarySnapshot`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StudyProbabilityLevel {
    /// Empirical probability (%) spot closes ABOVE `implied_price`.
    pub prob_above_pct: usize,
    /// Threshold return (%).
    pub return_pct: Decimal,
    /// Spot-anchored implied price level.
    pub implied_price: Decimal,
    /// Human-readable market scenario tag.
    pub scenario: String,
}

/// Concise, exact-`Decimal` statistical summary returned by [`StudyReader`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StudySummarySnapshot {
    /// Canonical analyzed instrument ID.
    pub instrument: InstrumentId,
    /// Active asset class classification (`index`, `fno_stock`, `etf`, `equity`, `forex`, `commodity`).
    pub asset_class: String,
    /// Boundary schedule applied (`weekly_expiry`, `monthly_expiry`, etc.).
    pub period_schedule: String,
    /// Return formula (`open_to_close`, `high_to_low`, `close_to_close`).
    pub calc_type: String,
    /// First trading bar date in evaluated range.
    pub start_date: NaiveDate,
    /// Last trading bar date in evaluated range.
    pub end_date: NaiveDate,
    /// Total sliced periods evaluated.
    pub total_periods: usize,
    /// Optional NSE/BSE regime transition note across window.
    pub regime_note: Option<String>,
    /// Mean period return (%).
    pub avg_return_pct: Decimal,
    /// Sample standard deviation (%).
    pub std_dev_pct: Decimal,
    /// Period Sharpe ratio.
    pub sharpe_ratio: Decimal,
    /// Period Sortino ratio.
    pub sortino_ratio: Decimal,
    /// Profit factor (`sum(+) / |sum(-)|`).
    pub profit_factor: Decimal,
    /// Largest positive period return (%).
    pub max_gain_pct: Decimal,
    /// Largest negative period return (%).
    pub max_loss_pct: Decimal,
    /// Share of positive periods (%).
    pub win_rate_pct: Decimal,
    /// Active streak polarity (`positive` / `negative`).
    pub current_streak_direction: Option<String>,
    /// Length of currently active streak.
    pub current_streak_length: usize,
    /// Compounded return of active streak (%).
    pub current_streak_return_pct: Decimal,
    /// Active 6-state Markov state label.
    pub current_markov_state: Option<String>,
    /// Strongest next-period Markov state label.
    pub strongest_next_markov_state: String,
    /// Empirical transition probability (%) of `strongest_next_markov_state`.
    pub strongest_next_probability_pct: Decimal,
    /// Key empirical exceedance levels (e.g., 80%, 90%, 95%) for strike sizing.
    pub probability_levels: Vec<StudyProbabilityLevel>,
    /// Lower 90% survivability outlier boundary (%).
    pub outlier_90_lower_pct: Decimal,
    /// Upper 90% survivability outlier boundary (%).
    pub outlier_90_upper_pct: Decimal,
    /// Total outlier periods beyond 90% survivability bounds.
    pub outlier_count: usize,
    /// Average next-period return (%) after large positive trigger periods.
    pub after_big_up_next_avg_pct: Option<Decimal>,
    /// Average next-period return (%) after large negative trigger periods.
    pub after_big_down_next_avg_pct: Option<Decimal>,
}

/// Read-only port supplying multi-asset `Experimental-Analysis` statistical summaries.
#[async_trait]
pub trait StudyReader: Send + Sync + 'static {
    /// Evaluates or fetches the empirical study summary for an instrument and schedule.
    async fn returns_study_summary(
        &self,
        instrument: &InstrumentId,
        asset_class: Option<&str>,
        period: Option<&str>,
        calc_type: Option<&str>,
        start_date: Option<NaiveDate>,
        end_date: Option<NaiveDate>,
    ) -> Result<StudySummarySnapshot, PortError>;
}
