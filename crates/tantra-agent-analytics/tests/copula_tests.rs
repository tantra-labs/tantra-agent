//! Tests for the bivariate copula tail-risk engine (plan 0007 Track 4 Phase 10).

use tantra_agent_analytics::{AnalyticsError, CopulaConfig, CopulaFamily, fit_bivariate_copula};

/// A deterministic near-uncorrelated pair of series.
fn independent_series() -> (Vec<f64>, Vec<f64>) {
    let x: Vec<f64> = (0..200).map(|i| (i as f64 * 0.13).sin()).collect();
    let y: Vec<f64> = (0..200).map(|i| (i as f64 * 0.29).cos() * 0.5).collect();
    (x, y)
}

/// A strongly concordant pair: y is x plus tiny jitter.
fn concordant_series() -> (Vec<f64>, Vec<f64>) {
    let x: Vec<f64> = (0..200).map(|i| (i as f64 * 0.31).sin()).collect();
    let y: Vec<f64> = x
        .iter()
        .enumerate()
        .map(|(i, &v)| v + 1e-6 * (i as f64).sin())
        .collect();
    (x, y)
}

#[test]
fn independent_series_low_tail_dependence() {
    let (x, y) = independent_series();
    let config = CopulaConfig {
        family: CopulaFamily::Clayton,
        n_simulations: 2000,
        seed: 42,
    };
    let result = fit_bivariate_copula(&x, &y, &config).expect("copula fits");
    assert!(
        result.theta > 1e-6,
        "theta must be floored positive, got {}",
        result.theta
    );
    assert!(
        result.theta < 3.0,
        "theta must stay small, got {}",
        result.theta
    );
    assert_eq!(result.tail_dependence_upper, 0.0);
    assert!(result.family == "clayton");
}

#[test]
fn concordant_series_positive_theta() {
    let (x, y) = concordant_series();
    let config = CopulaConfig {
        family: CopulaFamily::Clayton,
        n_simulations: 2000,
        seed: 42,
    };
    let result = fit_bivariate_copula(&x, &y, &config).expect("copula fits");
    assert!(result.kendall_tau > 0.5, "tau: {}", result.kendall_tau);
    assert!(result.theta > 3.0, "theta: {}", result.theta);
    assert!(result.tail_dependence_lower > 0.0);
}

#[test]
fn deterministic_seed() {
    let (x, y) = concordant_series();
    let config = CopulaConfig {
        family: CopulaFamily::Clayton,
        n_simulations: 1000,
        seed: 7,
    };
    let a = fit_bivariate_copula(&x, &y, &config).expect("first fit");
    let b = fit_bivariate_copula(&x, &y, &config).expect("second fit");
    assert_eq!(a, b);
}

#[test]
fn length_mismatch_errors() {
    let config = CopulaConfig::default();
    let x = vec![0.1, 0.2, 0.3, 0.4, 0.5];
    let y = vec![0.1, 0.2, 0.3];
    assert!(fit_bivariate_copula(&x, &y, &config).is_err());
}

#[test]
fn gumbel_upper_tail() {
    let (x, y) = concordant_series();
    let config = CopulaConfig {
        family: CopulaFamily::Gumbel,
        n_simulations: 1000,
        seed: 42,
    };
    let result = fit_bivariate_copula(&x, &y, &config).expect("copula fits");
    assert!(result.tail_dependence_upper > 0.0);
    assert_eq!(result.tail_dependence_lower, 0.0);
    assert!(result.family == "gumbel");
}

#[test]
fn too_short_errors() {
    let config = CopulaConfig::default();
    let x = vec![0.1, 0.2, 0.3];
    let y = vec![0.1, 0.2, 0.3];
    assert!(matches!(
        fit_bivariate_copula(&x, &y, &config),
        Err(AnalyticsError::InsufficientData { .. })
    ));
}
