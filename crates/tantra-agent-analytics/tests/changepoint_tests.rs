//! Tests for the change-point engines (plan 0007 Track 4 Phase 10).

use tantra_agent_analytics::{
    AnalyticsError, ChangePointConfig, detect_cusum_change_points, detect_structural_break,
};

/// 100 points of ~1.0 then 100 points of ~5.0 with tiny deterministic jitter.
fn shifted_series() -> Vec<f64> {
    let mut series = Vec::with_capacity(200);
    for i in 0..200 {
        let base = if i < 100 { 1.0 } else { 5.0 };
        series.push(base + (i as f64) * 1e-6);
    }
    series
}

/// Constant-with-tiny-jitter series (zero-mean jitter, no trend).
fn stable_series() -> Vec<f64> {
    (0..200)
        .map(|i| 1.0 + 1e-4 * ((i as f64) * 0.31).sin())
        .collect()
}

#[test]
fn detects_known_shift() {
    let series = shifted_series();
    let result =
        detect_cusum_change_points(&series, &ChangePointConfig::default()).expect("scan runs");
    assert!(result.detected, "break must be detected");
    assert!(!result.break_points.is_empty());
    // The shift is at index 100 (within ±15).
    let nearest = result
        .break_points
        .iter()
        .map(|&b| (b as i64 - 100).abs())
        .min()
        .expect("has a break");
    assert!(nearest <= 15, "nearest break distance: {nearest}");
}

#[test]
fn no_break_when_stable() {
    let series = stable_series();
    let result =
        detect_cusum_change_points(&series, &ChangePointConfig::default()).expect("scan runs");
    assert!(!result.detected, "stable series must not trigger a break");
    assert!(result.break_points.is_empty());
}

#[test]
fn structural_break_single() {
    let series = shifted_series();
    let idx = detect_structural_break(&series)
        .expect("statistic computes")
        .expect("a break must be found");
    assert!(
        (idx as i64 - 100).abs() <= 15,
        "structural break at {idx}, expected near 100"
    );
}

#[test]
fn empty_errors() {
    let config = ChangePointConfig::default();
    assert!(matches!(
        detect_cusum_change_points(&[], &config),
        Err(AnalyticsError::EmptyInput)
    ));
    assert!(matches!(
        detect_structural_break(&[]),
        Err(AnalyticsError::EmptyInput)
    ));
}

#[test]
fn determinism() {
    let series = shifted_series();
    let config = ChangePointConfig::default();
    let a = detect_cusum_change_points(&series, &config).expect("first run");
    let b = detect_cusum_change_points(&series, &config).expect("second run");
    assert_eq!(a, b);
    assert_eq!(
        detect_structural_break(&series).expect("first"),
        detect_structural_break(&series).expect("second")
    );
}
