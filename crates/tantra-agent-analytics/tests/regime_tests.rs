//! Tests for the Gaussian HMM regime engine (plan 0007 Track 4 Phase 9).

use tantra_agent_analytics::{AnalyticsError, HmmConfig, RegimeLabel, fit_gaussian_hmm};

/// Two well-separated blocks: 50 positive-mean then 50 negative-mean returns.
fn two_block_series() -> Vec<f64> {
    let mut returns = Vec::with_capacity(100);
    for i in 0..50 {
        returns.push(0.004 + 0.002 * ((i as f64) * 0.37).sin());
    }
    for i in 0..50 {
        returns.push(-0.004 + 0.002 * ((i as f64) * 0.37).sin());
    }
    returns
}

#[test]
fn two_state_hmm_separates_regimes() {
    let returns = two_block_series();
    let result = fit_gaussian_hmm(&returns, &HmmConfig::default()).expect("hmm fits");
    assert_eq!(result.states.len(), 100);
    assert_eq!(result.n_states, 2);
    assert_eq!(result.state_means.len(), 2);
    assert_eq!(result.transition_matrix.len(), 2);
    assert_eq!(result.transition_matrix[0].len(), 2);
    // State means are ordered ascending.
    assert!(result.state_means[0] < result.state_means[1]);
    // Majority of the first block labeled Bullish, last block Bearish.
    let bullish_first = result.states[..50]
        .iter()
        .filter(|&&s| s == RegimeLabel::Bullish)
        .count();
    let bearish_last = result.states[50..]
        .iter()
        .filter(|&&s| s == RegimeLabel::Bearish)
        .count();
    assert!(
        bullish_first > 25,
        "bullish in first block: {bullish_first}"
    );
    assert!(bearish_last > 25, "bearish in last block: {bearish_last}");
}

#[test]
fn three_state_hmm_works() {
    let mut returns = Vec::new();
    for i in 0..60 {
        returns.push(0.006 + 0.002 * ((i as f64) * 0.31).sin());
    }
    for i in 0..60 {
        returns.push(-0.006 + 0.002 * ((i as f64) * 0.31).sin());
    }
    for i in 0..60 {
        returns.push(0.004 * ((i as f64) * 0.47).sin());
    }
    let config = HmmConfig {
        n_states: 3,
        ..Default::default()
    };
    let result = fit_gaussian_hmm(&returns, &config).expect("hmm fits");
    assert_eq!(result.states.len(), 180);
    assert_eq!(result.state_means.len(), 3);
    assert!(result.state_means[0] <= result.state_means[1]);
    assert!(result.state_means[1] <= result.state_means[2]);
    let has_bullish = result.states.contains(&RegimeLabel::Bullish);
    let has_bearish = result.states.contains(&RegimeLabel::Bearish);
    let has_choppy = result.states.contains(&RegimeLabel::ChoppyVolatile);
    assert!(
        has_bullish && has_bearish && has_choppy,
        "all three regime labels should appear"
    );
}

#[test]
fn deterministic_repeatability() {
    let returns = two_block_series();
    let config = HmmConfig::default();
    let a = fit_gaussian_hmm(&returns, &config).expect("first fit");
    let b = fit_gaussian_hmm(&returns, &config).expect("second fit");
    assert_eq!(a, b);
}

#[test]
fn empty_input_errors() {
    let result = fit_gaussian_hmm(&[], &HmmConfig::default());
    assert!(matches!(result, Err(AnalyticsError::EmptyInput)));
}

#[test]
fn invalid_state_count_errors() {
    let returns = two_block_series();
    let config = HmmConfig {
        n_states: 4,
        ..Default::default()
    };
    let result = fit_gaussian_hmm(&returns, &config);
    assert!(matches!(result, Err(AnalyticsError::InvalidArgument(_))));
}
