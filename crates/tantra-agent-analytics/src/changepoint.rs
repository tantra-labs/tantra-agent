//! Change-point detection for structural breaks in time series (plan 0007
//! Track 4 Phase 10).
//!
//! Two deterministic engines are provided: a two-sided CUSUM scan
//! ([`detect_cusum_change_points`]) that finds every break by repeatedly
//! re-anchoring after a detected shift, and a single normalized mean-shift
//! statistic ([`detect_structural_break`]) that reports the strongest split.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::{AnalyticsError, Result};
use crate::math::{mean, variance};

/// Configuration for the CUSUM change-point scan.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ChangePointConfig {
    /// Minimum series length required to run the scan.
    pub min_observations: usize,
    /// Break threshold in units of `sqrt(n)` for the maximal |CUSUM|.
    pub threshold_multiplier: f64,
    /// Minimum distance (in observations) between recorded breaks, and from
    /// either end of the series.
    pub min_segment: usize,
}

impl Default for ChangePointConfig {
    fn default() -> Self {
        Self {
            min_observations: 10,
            threshold_multiplier: 3.0,
            min_segment: 5,
        }
    }
}

/// Result of a CUSUM change-point scan.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ChangePointResult {
    /// Indices (0-based) at which structural breaks were recorded.
    pub break_points: Vec<usize>,
    /// Largest |CUSUM| observed over the whole scan.
    pub cusum_max: f64,
    /// Whether at least one break was recorded.
    pub detected: bool,
}

/// Scans a series for structural breaks with a two-sided cumulative-sum
/// (CUSUM) statistic, re-anchoring at each detected break.
///
/// Each segment is standardized to its own mean/standard deviation; a break
/// is recorded at the argmax of |CUSUM| when that maximum exceeds
/// `threshold_multiplier * sqrt(segment_len)`, then the scan restarts on the
/// remaining tail. Breaks closer than `min_segment` to the previous break or
/// to either end are skipped.
///
/// # Errors
///
/// - [`AnalyticsError::EmptyInput`] on an empty series.
/// - [`AnalyticsError::InsufficientData`] when shorter than `min_observations`.
/// - [`AnalyticsError::ZeroVariance`] when the series has (near-)zero variance.
pub fn detect_cusum_change_points(
    series: &[f64],
    config: &ChangePointConfig,
) -> Result<ChangePointResult> {
    if series.is_empty() {
        return Err(AnalyticsError::EmptyInput);
    }
    if series.len() < config.min_observations {
        return Err(AnalyticsError::InsufficientData {
            needed: config.min_observations,
            got: series.len(),
        });
    }
    if variance(series) <= 1e-12 {
        return Err(AnalyticsError::ZeroVariance);
    }

    let n = series.len();
    let mut break_points = Vec::new();
    let mut cusum_max = 0.0f64;
    let mut start = 0usize;
    while start < n {
        let seg = &series[start..];
        if seg.len() < config.min_observations {
            break;
        }
        let v = variance(seg);
        if v <= 1e-12 {
            break;
        }
        let m = mean(seg);
        let s = v.sqrt();
        let threshold = config.threshold_multiplier * (seg.len() as f64).sqrt();
        let mut cusum = 0.0f64;
        let mut max_abs = 0.0f64;
        let mut max_idx = 0usize;
        for (i, &x) in seg.iter().enumerate() {
            cusum += (x - m) / s;
            let a = cusum.abs();
            if a > max_abs {
                max_abs = a;
                max_idx = i;
            }
            if a > cusum_max {
                cusum_max = a;
            }
        }
        if max_abs <= threshold {
            break;
        }
        let break_idx = start + max_idx;
        let far_from_prev = break_points
            .last()
            .is_none_or(|&p| break_idx - p >= config.min_segment);
        let far_from_start = break_idx >= config.min_segment;
        let far_from_end = n - break_idx > config.min_segment;
        if far_from_prev && far_from_start && far_from_end {
            break_points.push(break_idx);
        }
        start = break_idx + 1;
    }

    let detected = !break_points.is_empty();
    Ok(ChangePointResult {
        break_points,
        cusum_max,
        detected,
    })
}

/// Reports the strongest normalized mean-shift split of a series, if any.
///
/// For every split `i` in `[min_segment, n - min_segment)` the statistic
/// `|mean(left) - mean(right)| * sqrt(n_left * n_right / n_total) / std` is
/// computed (a scale-invariant two-sample separation); `Some(i)` is returned
/// at the argmax when the score exceeds the default threshold of `3.0`.
///
/// # Errors
///
/// - [`AnalyticsError::EmptyInput`] on an empty series.
/// - [`AnalyticsError::ZeroVariance`] when the series has (near-)zero variance.
pub fn detect_structural_break(series: &[f64]) -> Result<Option<usize>> {
    let n = series.len();
    if n == 0 {
        return Err(AnalyticsError::EmptyInput);
    }
    let v = variance(series);
    if v <= 1e-12 {
        return Err(AnalyticsError::ZeroVariance);
    }
    let config = ChangePointConfig::default();
    if n < 2 * config.min_segment {
        return Ok(None);
    }
    let std = v.sqrt();
    let mut prefix = vec![0.0f64; n + 1];
    for (i, &x) in series.iter().enumerate() {
        prefix[i + 1] = prefix[i] + x;
    }
    let mut best: Option<usize> = None;
    let mut best_score = f64::NEG_INFINITY;
    for i in config.min_segment..(n - config.min_segment) {
        let n_left = i as f64;
        let n_right = (n - i) as f64;
        let m_left = prefix[i] / n_left;
        let m_right = (prefix[n] - prefix[i]) / n_right;
        let score = (m_left - m_right).abs() * (n_left * n_right / n as f64).sqrt() / std;
        if score > best_score {
            best_score = score;
            best = Some(i);
        }
    }
    if best_score > config.threshold_multiplier {
        Ok(best)
    } else {
        Ok(None)
    }
}
