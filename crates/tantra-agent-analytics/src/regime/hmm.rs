//! Gaussian hidden Markov model (HMM) regime detection.
//!
//! Fits a univariate Gaussian HMM to a return series with the Baum-Welch
//! expectation-maximization (EM) algorithm and decodes the most likely state
//! sequence with Viterbi. The fit is fully deterministic — there is no RNG:
//! initial parameters are derived from sorted-return quantile bins, so
//! identical inputs always produce a byte-identical [`HmmResult`].

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::{AnalyticsError, Result};
use crate::math::{mean, variance};

/// Configuration for the Gaussian HMM fit.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct HmmConfig {
    /// Number of hidden regimes (2 or 3).
    pub n_states: usize,
    /// Maximum number of Baum-Welch EM iterations.
    pub max_iterations: usize,
    /// Log-likelihood improvement threshold for EM convergence.
    pub tolerance: f64,
    /// Reserved for future stochastic initialization. Currently unused, but
    /// retained for API stability so callers can pin a deterministic seed
    /// without the signature changing when randomized seeding lands.
    pub seed: u64,
}

impl Default for HmmConfig {
    fn default() -> Self {
        Self {
            n_states: 2,
            max_iterations: 100,
            tolerance: 1e-6,
            seed: 42,
        }
    }
}

/// A labeled market regime.
#[derive(Clone, Copy, Debug, PartialEq, Eq, JsonSchema)]
pub enum RegimeLabel {
    /// Above-average return regime (highest mean).
    Bullish,
    /// Below-average return regime (lowest mean).
    Bearish,
    /// Middle return regime (intermediate mean).
    ChoppyVolatile,
}

impl std::fmt::Display for RegimeLabel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegimeLabel::Bullish => write!(f, "bullish"),
            RegimeLabel::Bearish => write!(f, "bearish"),
            RegimeLabel::ChoppyVolatile => write!(f, "choppy_volatile"),
        }
    }
}

impl Serialize for RegimeLabel {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for RegimeLabel {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "bullish" => Ok(RegimeLabel::Bullish),
            "bearish" => Ok(RegimeLabel::Bearish),
            "choppy_volatile" => Ok(RegimeLabel::ChoppyVolatile),
            other => Err(serde::de::Error::unknown_variant(
                other,
                &["bullish", "bearish", "choppy_volatile"],
            )),
        }
    }
}

/// Result of a Gaussian HMM regime fit.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct HmmResult {
    /// Number of fitted regimes.
    pub n_states: usize,
    /// One regime label per observation (Viterbi decoding), labeled by mean order.
    pub states: Vec<RegimeLabel>,
    /// Estimated mean return per state, ascending.
    pub state_means: Vec<f64>,
    /// Estimated return variance per state (ascending-mean order).
    pub state_variances: Vec<f64>,
    /// Fitted state transition matrix, `n_states x n_states`, rows sum to 1.
    pub transition_matrix: Vec<Vec<f64>>,
    /// Final log-likelihood of the observations under the fitted model.
    pub log_likelihood: f64,
    /// Whether EM converged (improvement below `tolerance`) before `max_iterations`.
    pub converged: bool,
}

/// Fits a Gaussian HMM to a return series and decodes per-observation regimes.
///
/// # Errors
///
/// - [`AnalyticsError::EmptyInput`] when `returns` is empty.
/// - [`AnalyticsError::InvalidArgument`] when `n_states` is not 2 or 3, or
///   `max_iterations` is zero.
/// - [`AnalyticsError::InsufficientData`] when `returns` is shorter than
///   `2 * n_states`.
pub fn fit_gaussian_hmm(returns: &[f64], config: &HmmConfig) -> Result<HmmResult> {
    if returns.is_empty() {
        return Err(AnalyticsError::EmptyInput);
    }
    if config.n_states != 2 && config.n_states != 3 {
        return Err(AnalyticsError::InvalidArgument(format!(
            "n_states must be 2 or 3, got {}",
            config.n_states
        )));
    }
    if config.max_iterations == 0 {
        return Err(AnalyticsError::InvalidArgument(
            "max_iterations must be at least 1".to_string(),
        ));
    }
    let k = config.n_states;
    let t = returns.len();
    if t < k * 2 {
        return Err(AnalyticsError::InsufficientData {
            needed: k * 2,
            got: t,
        });
    }

    // Deterministic initialization from sorted-return quantile bins.
    let mut sorted = returns.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut means = Vec::with_capacity(k);
    let mut variances = Vec::with_capacity(k);
    for i in 0..k {
        let start = i * t / k;
        let end = (i + 1) * t / k;
        let bin = &sorted[start..end];
        let m = mean(bin);
        let mut v = variance(bin);
        if !v.is_finite() || v <= 0.0 {
            v = 1e-8;
        }
        means.push(m);
        variances.push(v);
    }
    let prior = vec![1.0 / k as f64; k];
    let transition = vec![vec![1.0 / k as f64; k]; k];

    let (log_likelihood, converged, fitted_means, fitted_variances, fitted_transition) =
        baum_welch(returns, k, &means, &variances, &prior, &transition, config);
    let decoded = viterbi_decode(
        returns,
        k,
        &fitted_means,
        &fitted_variances,
        &prior,
        &fitted_transition,
    );

    // Label and reorder states by ascending fitted mean.
    let mut order: Vec<usize> = (0..k).collect();
    order.sort_by(|&a, &b| {
        fitted_means[a]
            .partial_cmp(&fitted_means[b])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let labels = label_states(&order);
    let states: Vec<RegimeLabel> = decoded.iter().map(|&s| labels[s]).collect();
    let state_means: Vec<f64> = order.iter().map(|&i| fitted_means[i]).collect();
    let state_variances: Vec<f64> = order.iter().map(|&i| fitted_variances[i]).collect();
    let mut transition_matrix = vec![vec![0.0f64; k]; k];
    for (r, &i) in order.iter().enumerate() {
        for (c, &j) in order.iter().enumerate() {
            transition_matrix[r][c] = fitted_transition[i][j];
        }
    }

    Ok(HmmResult {
        n_states: k,
        states,
        state_means,
        state_variances,
        transition_matrix,
        log_likelihood,
        converged,
    })
}

/// Maps a state index (by ascending-mean order) to its [`RegimeLabel`].
fn label_states(order: &[usize]) -> Vec<RegimeLabel> {
    let k = order.len();
    let mut labels = vec![RegimeLabel::ChoppyVolatile; k];
    labels[order[0]] = RegimeLabel::Bearish;
    labels[order[k - 1]] = RegimeLabel::Bullish;
    if k == 3 {
        labels[order[1]] = RegimeLabel::ChoppyVolatile;
    }
    labels
}

/// Log density of a univariate Gaussian.
fn log_emit(o: f64, m: f64, v: f64) -> f64 {
    -0.5 * (std::f64::consts::TAU.ln() + v.ln() + (o - m).powi(2) / v)
}

/// Log-sum-exp over a slice (numerically stable).
fn log_sum_exp(xs: &[f64]) -> f64 {
    let max = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if !max.is_finite() {
        return f64::NEG_INFINITY;
    }
    max + xs.iter().map(|&x| (x - max).exp()).sum::<f64>().ln()
}

/// Baum-Welch EM loop, returning (log-likelihood, converged, means, variances, transition).
#[allow(clippy::too_many_arguments)]
fn baum_welch(
    obs: &[f64],
    k: usize,
    init_means: &[f64],
    init_variances: &[f64],
    init_prior: &[f64],
    init_transition: &[Vec<f64>],
    config: &HmmConfig,
) -> (f64, bool, Vec<f64>, Vec<f64>, Vec<Vec<f64>>) {
    let t = obs.len();
    let mut means = init_means.to_vec();
    let mut variances = init_variances.to_vec();
    let mut prior = init_prior.to_vec();
    let mut transition = init_transition.to_vec();
    let mut log_likelihood = f64::NEG_INFINITY;
    let mut converged = false;

    for _ in 0..config.max_iterations {
        // Forward pass (log-domain).
        let mut log_alpha = vec![vec![0.0f64; k]; t];
        for j in 0..k {
            log_alpha[0][j] = prior[j].ln() + log_emit(obs[0], means[j], variances[j]);
        }
        for tt in 1..t {
            for j in 0..k {
                let mut acc = Vec::with_capacity(k);
                for i in 0..k {
                    acc.push(log_alpha[tt - 1][i] + transition[i][j].ln());
                }
                log_alpha[tt][j] = log_sum_exp(&acc) + log_emit(obs[tt], means[j], variances[j]);
            }
        }
        let new_ll = log_sum_exp(&log_alpha[t - 1]);

        // Backward pass (log-domain).
        let mut log_beta = vec![vec![0.0f64; k]; t];
        for tt in (0..t - 1).rev() {
            for i in 0..k {
                let mut acc = Vec::with_capacity(k);
                for j in 0..k {
                    acc.push(
                        transition[i][j].ln()
                            + log_emit(obs[tt + 1], means[j], variances[j])
                            + log_beta[tt + 1][j],
                    );
                }
                log_beta[tt][i] = log_sum_exp(&acc);
            }
        }

        // E-step: responsibilities gamma and xi.
        let mut gamma = vec![vec![0.0f64; k]; t];
        for tt in 0..t {
            for j in 0..k {
                gamma[tt][j] = (log_alpha[tt][j] + log_beta[tt][j] - new_ll).exp();
            }
        }
        let mut xi = vec![vec![vec![0.0f64; k]; k]; t.saturating_sub(1)];
        for tt in 0..t.saturating_sub(1) {
            for i in 0..k {
                for j in 0..k {
                    xi[tt][i][j] = (log_alpha[tt][i]
                        + transition[i][j].ln()
                        + log_emit(obs[tt + 1], means[j], variances[j])
                        + log_beta[tt + 1][j]
                        - new_ll)
                        .exp();
                }
            }
        }

        // M-step: initial state distribution from gamma at t = 0.
        prior.copy_from_slice(&gamma[0]);
        let prior_sum: f64 = prior.iter().sum();
        if prior_sum > 0.0 {
            for p in &mut prior {
                *p /= prior_sum;
            }
        }

        // M-step: transition matrix.
        let mut new_transition = vec![vec![0.0f64; k]; k];
        for i in 0..k {
            let denom: f64 = (0..t.saturating_sub(1)).map(|tt| gamma[tt][i]).sum();
            for j in 0..k {
                let numer: f64 = (0..t.saturating_sub(1)).map(|tt| xi[tt][i][j]).sum();
                new_transition[i][j] = if denom > 1e-12 {
                    numer / denom
                } else {
                    transition[i][j]
                };
            }
        }
        for row in &mut new_transition {
            let s: f64 = row.iter().sum();
            if s > 0.0 {
                for v in row.iter_mut() {
                    *v /= s;
                }
            }
        }
        transition = new_transition;

        // M-step: Gaussian parameters.
        for j in 0..k {
            let denom: f64 = (0..t).map(|tt| gamma[tt][j]).sum();
            if denom > 1e-12 {
                let numer: f64 = (0..t).map(|tt| gamma[tt][j] * obs[tt]).sum();
                means[j] = numer / denom;
                let scatter: f64 = (0..t)
                    .map(|tt| gamma[tt][j] * (obs[tt] - means[j]).powi(2))
                    .sum();
                variances[j] = (scatter / denom).max(1e-8);
            }
        }

        let improvement = new_ll - log_likelihood;
        log_likelihood = new_ll;
        if improvement.abs() < config.tolerance {
            converged = true;
            break;
        }
    }

    (log_likelihood, converged, means, variances, transition)
}

/// Viterbi decoding of the most likely state sequence.
fn viterbi_decode(
    obs: &[f64],
    k: usize,
    means: &[f64],
    variances: &[f64],
    prior: &[f64],
    transition: &[Vec<f64>],
) -> Vec<usize> {
    let t = obs.len();
    let mut delta = vec![vec![f64::NEG_INFINITY; k]; t];
    let mut psi = vec![vec![0usize; k]; t];
    for j in 0..k {
        delta[0][j] = prior[j].ln() + log_emit(obs[0], means[j], variances[j]);
    }
    for tt in 1..t {
        for j in 0..k {
            let mut best = f64::NEG_INFINITY;
            let mut best_i = 0usize;
            for i in 0..k {
                let candidate = delta[tt - 1][i] + transition[i][j].ln();
                if candidate > best {
                    best = candidate;
                    best_i = i;
                }
            }
            delta[tt][j] = best + log_emit(obs[tt], means[j], variances[j]);
            psi[tt][j] = best_i;
        }
    }
    let mut states = vec![0usize; t];
    let mut best = f64::NEG_INFINITY;
    for (j, &value) in delta[t - 1].iter().enumerate() {
        if value > best {
            best = value;
            states[t - 1] = j;
        }
    }
    for tt in (0..t - 1).rev() {
        states[tt] = psi[tt + 1][states[tt + 1]];
    }
    states
}
