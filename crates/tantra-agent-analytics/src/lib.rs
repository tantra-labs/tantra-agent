#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(unsafe_code)]

//! Performance analytics for the Tantra platform.
//!
//! Everything here takes injected data: round trips for trade statistics,
//! return or equity series for risk metrics, and explicit annualization
//! parameters. Nothing is hardcoded.
//!
//! # Example
//!
//! ```
//! use tantra_agent_analytics::{EquityStats, TradeStats};
//!
//! let returns = [0.01, -0.005, 0.02, -0.003, 0.015];
//! let stats = EquityStats::from_returns(&returns, 252.0, 0.0).unwrap();
//! assert!(stats.total_return > 0.0);
//!
//! // Trade stats need at least one round trip; see `RoundTrip`.
//! let _ = TradeStats::from_round_trips(&[]).is_err();
//! ```

pub mod changepoint;
pub mod cointegration;
pub mod copula;
pub mod cost_stress;
pub mod dsr;
pub mod equity_stats;
pub mod error;
pub mod factor_attribution;
pub mod holdout;
pub mod math;
pub mod monte_carlo;
pub mod pbo;
pub mod regime;
pub mod report;
pub mod round_trip;
pub mod trade_stats;
pub mod validation;
pub mod volatility;
pub mod walk_forward;

pub use changepoint::{
    ChangePointConfig, ChangePointResult, detect_cusum_change_points, detect_structural_break,
};
pub use cointegration::{CointegrationResult, test_engle_granger};
pub use copula::{CopulaConfig, CopulaFamily, CopulaResult, fit_bivariate_copula};
pub use cost_stress::{CostStressConfig, CostStressResult, compute_cost_stress};
pub use dsr::{DsrConfig, DsrResult, compute_dsr, expected_max_sharpe};
pub use equity_stats::EquityStats;
pub use error::{AnalyticsError, Result};
pub use factor_attribution::{
    FactorAttribution, FactorContribution, FactorReturnPanel, PortfolioHolding, SectorExposure,
    StyleExposure, compute_factor_attribution, factor_attribution_blocks,
    fit_cross_sectional_factor_model,
};
pub use holdout::{HoldoutConfig, HoldoutResult, compute_holdout};
pub use monte_carlo::{
    MonteCarloConfig, MonteCarloResult, run_block_bootstrap, simulate_bootstrap,
};
pub use pbo::{PboConfig, PboResult, compute_pbo};
pub use regime::{HmmConfig, HmmResult, RegimeLabel, fit_gaussian_hmm};
pub use report::PerformanceReport;
pub use round_trip::RoundTrip;
pub use trade_stats::TradeStats;
pub use validation::{
    PermutationTestConfig, PermutationTestResult, ValidationBlock, permutation_test_sharpe,
    permutation_test_win_rate,
};
pub use volatility::{GarchConfig, GarchResult, fit_garch11};
pub use walk_forward::{
    WalkForwardConfig, WalkForwardResult, compute_walk_forward, evaluate_walk_forward,
};
