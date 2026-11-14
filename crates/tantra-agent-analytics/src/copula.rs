//! Bivariate copula tail-risk estimation (plan 0007 Track 4 Phase 10).
//!
//! Fits an Archimedean copula (Clayton or Gumbel) to a return pair via
//! Kendall's tau, reports the analytic tail-dependence coefficients, and
//! estimates a joint 95% VaR with a seeded non-parametric bootstrap.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::{AnalyticsError, Result};
use crate::math::SplitMix64;
use crate::math::variance;

/// The Archimedean copula family to fit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum CopulaFamily {
    /// Clayton copula: lower-tail dependence.
    Clayton,
    /// Gumbel copula: upper-tail dependence.
    Gumbel,
}

impl std::fmt::Display for CopulaFamily {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CopulaFamily::Clayton => write!(f, "clayton"),
            CopulaFamily::Gumbel => write!(f, "gumbel"),
        }
    }
}

/// Configuration for the copula fit and joint VaR bootstrap.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CopulaConfig {
    /// Copula family to fit.
    pub family: CopulaFamily,
    /// Number of bootstrap draws for the joint VaR estimate.
    pub n_simulations: usize,
    /// Deterministic seed for the bootstrap (reproducible results).
    pub seed: u64,
}

impl Default for CopulaConfig {
    fn default() -> Self {
        Self {
            family: CopulaFamily::Clayton,
            n_simulations: 5000,
            seed: 42,
        }
    }
}

/// Result of a bivariate copula tail-risk fit.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CopulaResult {
    /// Fitted copula family ("clayton" or "gumbel").
    pub family: String,
    /// Fitted copula dependence parameter theta.
    pub theta: f64,
    /// Kendall's rank correlation tau.
    pub kendall_tau: f64,
    /// Upper tail-dependence coefficient (0.0 for Clayton).
    pub tail_dependence_upper: f64,
    /// Lower tail-dependence coefficient (0.0 for Gumbel).
    pub tail_dependence_lower: f64,
    /// Joint return `(x + y) / 2` at the 5th percentile of the seeded
    /// bootstrap distribution — a negative value means a loss (sign
    /// convention: return, not loss, so `-0.031` is a 5% loss).
    pub joint_var_95: f64,
}

/// Fits an Archimedean copula to a bivariate return series.
///
/// Pseudo-observations are formed from average ranks, Kendall's tau is
/// computed over all pairs, and theta is derived in closed form. The joint
/// 95% VaR is estimated by a seeded non-parametric bootstrap over the observed
/// pairs, so results are deterministic for a fixed [`CopulaConfig::seed`].
///
/// # Errors
///
/// - [`AnalyticsError::EmptyInput`] when the series lengths differ or are empty.
/// - [`AnalyticsError::InsufficientData`] when fewer than 4 observations.
/// - [`AnalyticsError::ZeroVariance`] when either series has (near-)zero variance.
pub fn fit_bivariate_copula(x: &[f64], y: &[f64], config: &CopulaConfig) -> Result<CopulaResult> {
    if x.is_empty() || y.is_empty() || x.len() != y.len() {
        return Err(AnalyticsError::EmptyInput);
    }
    let n = x.len();
    if n < 4 {
        return Err(AnalyticsError::InsufficientData { needed: 4, got: n });
    }
    if variance(x) <= 1e-12 || variance(y) <= 1e-12 {
        return Err(AnalyticsError::ZeroVariance);
    }

    // Pseudo-observations via average ascending ranks (validated in range).
    let u = ranks(x);
    let v = ranks(y);
    debug_assert!(u.iter().all(|&r| r > 0.0 && r < 1.0));
    debug_assert!(v.iter().all(|&r| r > 0.0 && r < 1.0));

    // Kendall's tau over all pairs.
    let pairs = (n * (n - 1) / 2) as f64;
    let mut concordant = 0.0f64;
    let mut discordant = 0.0f64;
    for i in 0..n {
        for j in (i + 1)..n {
            let d = (x[i] - x[j]) * (y[i] - y[j]);
            if d > 0.0 {
                concordant += 1.0;
            } else if d < 0.0 {
                discordant += 1.0;
            }
        }
    }
    let tau = (concordant - discordant) / pairs;

    // Guard against the degenerate tau -> +/-1 boundary: a perfectly
    // concordant (or discordant) sample would make theta infinite, which
    // cannot be represented in the JSON envelope.
    let tau_clamped = tau.clamp(-0.9999, 0.9999);
    let theta = match config.family {
        CopulaFamily::Clayton => (2.0 * tau_clamped / (1.0 - tau_clamped)).max(1e-6),
        CopulaFamily::Gumbel => (1.0 / (1.0 - tau_clamped)).max(1.0 + 1e-6),
    };
    let (tail_dependence_lower, tail_dependence_upper) = match config.family {
        CopulaFamily::Clayton => (2.0f64.powf(-1.0 / theta), 0.0),
        CopulaFamily::Gumbel => (0.0, 2.0 - 2.0f64.powf(1.0 / theta)),
    };

    // Seeded non-parametric bootstrap for the joint VaR95.
    let mut rng = SplitMix64::new(config.seed);
    let mut joint = Vec::with_capacity(config.n_simulations);
    for _ in 0..config.n_simulations {
        let i = rng.next_usize(n);
        joint.push((x[i] + y[i]) / 2.0);
    }
    joint.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let idx = ((config.n_simulations as f64) * 0.05)
        .floor()
        .min(config.n_simulations.saturating_sub(1) as f64) as usize;
    let joint_var_95 = joint[idx];

    Ok(CopulaResult {
        family: config.family.to_string(),
        theta,
        kendall_tau: tau,
        tail_dependence_upper,
        tail_dependence_lower,
        joint_var_95,
    })
}

/// 1-based average ascending ranks normalized by `1 / (n + 1)`.
fn ranks(xs: &[f64]) -> Vec<f64> {
    let n = xs.len();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| {
        xs[a]
            .partial_cmp(&xs[b])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut out = vec![0.0f64; n];
    let mut i = 0usize;
    while i < n {
        let mut j = i;
        while j + 1 < n && xs[order[j + 1]] == xs[order[i]] {
            j += 1;
        }
        let avg = (i + j) as f64 / 2.0 + 1.0;
        for &k in &order[i..=j] {
            out[k] = avg / (n as f64 + 1.0);
        }
        i = j + 1;
    }
    out
}
