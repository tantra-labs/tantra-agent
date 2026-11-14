//! Tests for the permutation-test engines (plan 0007 Track 4 Phase 10).

use tantra_agent_analytics::{
    AnalyticsError, PermutationTestConfig, permutation_test_sharpe, permutation_test_win_rate,
};

fn strongly_positive_returns() -> Vec<f64> {
    (0..50)
        .map(|i| 0.02 + 0.001 * ((i as f64) * 0.31).sin())
        .collect()
}

#[test]
fn deterministic() {
    let returns = strongly_positive_returns();
    let config = PermutationTestConfig::default();
    let a = permutation_test_sharpe(&returns, &config).expect("first run");
    let b = permutation_test_sharpe(&returns, &config).expect("second run");
    assert_eq!(a, b);
}

#[test]
fn edge_series_significant() {
    let returns = strongly_positive_returns();
    let config = PermutationTestConfig {
        n_permutations: 500,
        seed: 42,
    };
    let result = permutation_test_sharpe(&returns, &config).expect("test runs");
    assert!(result.observed_stat > 0.0);
    assert!(
        result.significant,
        "strongly positive returns should be significant, p={}",
        result.p_value
    );
}

#[test]
fn p_value_in_range() {
    let returns = strongly_positive_returns();
    let config = PermutationTestConfig::default();
    let result = permutation_test_sharpe(&returns, &config).expect("test runs");
    assert!((0.0..=1.0).contains(&result.p_value));
}

#[test]
fn win_rate_test() {
    let mut trades: Vec<f64> = vec![1.0; 30];
    trades.extend(vec![-1.0; 10]);
    let config = PermutationTestConfig {
        n_permutations: 300,
        seed: 42,
    };
    let result = permutation_test_win_rate(&trades, &config).expect("test runs");
    assert!(
        result.observed_stat > 0.5,
        "win rate: {}",
        result.observed_stat
    );
    assert_eq!(result.n_permutations, 300);
}

#[test]
fn empty_errors() {
    let config = PermutationTestConfig::default();
    assert!(matches!(
        permutation_test_sharpe(&[], &config),
        Err(AnalyticsError::EmptyInput)
    ));
    assert!(matches!(
        permutation_test_win_rate(&[], &config),
        Err(AnalyticsError::EmptyInput)
    ));
}
