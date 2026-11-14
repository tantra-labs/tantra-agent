//! Unified overfitting validation block and statistical gate evaluation (E4-S6).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::cost_stress::{CostStressConfig, CostStressResult, compute_cost_stress};
use crate::dsr::{DsrConfig, DsrResult, compute_dsr};
use crate::error::AnalyticsError;
use crate::holdout::{HoldoutConfig, HoldoutResult, compute_holdout};
use crate::math::{SplitMix64, mean, variance};
use crate::monte_carlo::{MonteCarloConfig, MonteCarloResult, run_block_bootstrap};
use crate::pbo::{PboConfig, PboResult, compute_pbo};
use crate::walk_forward::{WalkForwardConfig, WalkForwardResult, compute_walk_forward};

/// Comprehensive statistical validation block for a backtest or parameter sweep.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ValidationBlock {
    /// Deflated Sharpe Ratio validation result.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dsr: Option<DsrResult>,
    /// Probability of Backtest Overfitting validation result (for multi-trial sweeps).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pbo: Option<PboResult>,
    /// Walk-Forward Efficiency validation result.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub walk_forward: Option<WalkForwardResult>,
    /// One-shot holdout validation result.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub holdout: Option<HoldoutResult>,
    /// Block-bootstrap Monte Carlo drawdown and ruin simulation result.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub monte_carlo: Option<MonteCarloResult>,
    /// Cost-stress friction sensitivity result.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_stress: Option<CostStressResult>,
    /// Overall verdict: true only if all evaluated gates passed.
    pub passed: bool,
    /// Human-readable warnings and failure reasons.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

impl ValidationBlock {
    /// Evaluates all overfitting gates on a single backtest return series.
    pub fn evaluate_backtest(returns: &[f64], observed_sharpe: f64, trial_count: usize) -> Self {
        let mut warnings = Vec::new();

        // 1. DSR
        let dsr_cfg = DsrConfig {
            num_trials: trial_count.max(1),
            ..Default::default()
        };
        let dsr = Some(compute_dsr(observed_sharpe, returns, dsr_cfg));
        if let Some(d) = &dsr {
            if !d.passed {
                warnings.push(format!(
                    "DSR failed: observed Sharpe {:.2} deflated to {:.1}% (threshold: 95.0%, SR0: {:.2})",
                    observed_sharpe,
                    d.dsr * 100.0,
                    d.expected_max_sharpe
                ));
            }
        }

        // 2. Walk-Forward
        let wf = Some(compute_walk_forward(returns, WalkForwardConfig::default()));
        if let Some(w) = &wf {
            if !w.passed {
                warnings.push(format!(
                    "Walk-Forward Efficiency failed: WFE {:.1}% (min required: 50.0%)",
                    w.wfe * 100.0
                ));
            }
        }

        // 3. One-shot Holdout
        let ho_cfg = HoldoutConfig {
            trial_count: trial_count.max(1),
            ..Default::default()
        };
        let holdout = Some(compute_holdout(returns, ho_cfg));
        if let Some(h) = &holdout {
            if !h.passed {
                warnings.push(format!(
                    "Holdout validation failed: retention {:.1}% (min: 50.0%, holdout Sharpe: {:.2})",
                    h.retention_ratio * 100.0,
                    h.holdout_sharpe
                ));
            }
        }

        // 4. Block-Bootstrap Monte Carlo
        let mc = Some(run_block_bootstrap(returns, MonteCarloConfig::default()));
        if let Some(m) = &mc {
            if !m.passed {
                warnings.push(format!(
                    "Monte Carlo bootstrap failed: 95% worst DD {:.1}% (threshold: 30.0%, ruin prob: {:.1}%)",
                    m.drawdown_95 * 100.0,
                    m.prob_ruin * 100.0
                ));
            }
        }

        // 5. Cost-Stress Testing
        let cs = Some(compute_cost_stress(returns, CostStressConfig::default()));
        if let Some(c) = &cs {
            if !c.passed {
                warnings.push(format!(
                    "Cost-stress failed: 2x friction Sharpe {:.2} collapsed below 0.0",
                    c.stress_2x_sharpe
                ));
            }
        }

        let passed = warnings.is_empty();

        Self {
            dsr,
            pbo: None, // PBO is evaluated across multi-trial sweeps
            walk_forward: wf,
            holdout,
            monte_carlo: mc,
            cost_stress: cs,
            passed,
            warnings,
        }
    }

    /// Evaluates overfitting gates across a multi-trial sweep.
    pub fn evaluate_sweep(
        best_returns: &[f64],
        best_sharpe: f64,
        trial_returns: &[Vec<f64>],
    ) -> Self {
        let mut block = Self::evaluate_backtest(best_returns, best_sharpe, trial_returns.len());

        // Add PBO across all sweep trials
        if trial_returns.len() >= 2 {
            let pbo_res = compute_pbo(trial_returns, PboConfig::default());
            if !pbo_res.passed {
                block.warnings.push(format!(
                    "PBO failed: Probability of Backtest Overfitting {:.1}% exceeds 50.0%",
                    pbo_res.pbo * 100.0
                ));
                block.passed = false;
            }
            block.pbo = Some(pbo_res);
        }

        block
    }
}

// ---------------------------------------------------------------------------
// Permutation tests (plan 0007 Track 4 Phase 10)
// ---------------------------------------------------------------------------

/// Configuration for a seeded permutation test.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PermutationTestConfig {
    /// Number of permutations to evaluate.
    pub n_permutations: usize,
    /// Deterministic seed for the permutation RNG.
    pub seed: u64,
}

impl Default for PermutationTestConfig {
    fn default() -> Self {
        Self {
            n_permutations: 1000,
            seed: 42,
        }
    }
}

/// Result of a permutation test.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PermutationTestResult {
    /// The observed test statistic (Sharpe or win rate).
    pub observed_stat: f64,
    /// Permutation p-value in `[0, 1]`.
    pub p_value: f64,
    /// Number of permutations evaluated.
    pub n_permutations: usize,
    /// Whether the result is significant at the 5% level (`p_value < 0.05`).
    pub significant: bool,
}

/// Permutation test on the (non-annualized) Sharpe ratio `mean / std`.
///
/// Each permutation flips every return's sign with probability 0.5 (a seeded
/// [`SplitMix64`] draw) and recomputes the Sharpe ratio; the p-value counts
/// permutations at least as extreme as the observed statistic:
/// `p = (1 + #(|perm| >= |observed|)) / (1 + n_permutations)`.
///
/// # Errors
///
/// - [`AnalyticsError::EmptyInput`] on an empty series.
/// - [`AnalyticsError::ZeroVariance`] when the series has (near-)zero variance.
pub fn permutation_test_sharpe(
    returns: &[f64],
    config: &PermutationTestConfig,
) -> crate::Result<PermutationTestResult> {
    if returns.is_empty() {
        return Err(AnalyticsError::EmptyInput);
    }
    let v = variance(returns);
    if v <= 1e-12 {
        return Err(AnalyticsError::ZeroVariance);
    }
    let observed = mean(returns) / v.sqrt();

    let mut rng = SplitMix64::new(config.seed);
    let mut perm = vec![0.0f64; returns.len()];
    let mut at_least_as_extreme = 0u64;
    for _ in 0..config.n_permutations {
        for (dst, &src) in perm.iter_mut().zip(returns) {
            *dst = if rng.next_f64() < 0.5 { -src } else { src };
        }
        let pv = variance(&perm);
        if pv <= 1e-12 {
            continue;
        }
        let ps = mean(&perm) / pv.sqrt();
        if ps.abs() >= observed.abs() {
            at_least_as_extreme += 1;
        }
    }
    let p_value = (1.0 + at_least_as_extreme as f64) / (config.n_permutations as f64 + 1.0);
    Ok(PermutationTestResult {
        observed_stat: observed,
        p_value,
        n_permutations: config.n_permutations,
        significant: p_value < 0.05,
    })
}

/// Permutation test on the win rate `#(trades > 0) / n`.
///
/// Each permutation is a seeded Fisher-Yates shuffle of the trades; the
/// p-value counts permutations with a win rate at least as high as observed:
/// `p = (1 + #(perm >= observed)) / (1 + n_permutations)`.
///
/// # Errors
///
/// - [`AnalyticsError::EmptyInput`] on an empty series.
pub fn permutation_test_win_rate(
    trades: &[f64],
    config: &PermutationTestConfig,
) -> crate::Result<PermutationTestResult> {
    if trades.is_empty() {
        return Err(AnalyticsError::EmptyInput);
    }
    let n = trades.len();
    let observed = trades.iter().filter(|&&t| t > 0.0).count() as f64 / n as f64;

    let mut rng = SplitMix64::new(config.seed);
    let mut perm = trades.to_vec();
    let mut at_least_as_high = 0u64;
    for _ in 0..config.n_permutations {
        for i in (1..n).rev() {
            let j = rng.next_usize(i + 1);
            perm.swap(i, j);
        }
        let wr = perm.iter().filter(|&&t| t > 0.0).count() as f64 / n as f64;
        if wr >= observed {
            at_least_as_high += 1;
        }
    }
    let p_value = (1.0 + at_least_as_high as f64) / (config.n_permutations as f64 + 1.0);
    Ok(PermutationTestResult {
        observed_stat: observed,
        p_value,
        n_permutations: config.n_permutations,
        significant: p_value < 0.05,
    })
}
