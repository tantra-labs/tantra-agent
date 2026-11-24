//! Unit tests for GARCH(1,1) volatility estimation.

use tantra_agent_analytics::{AnalyticsError, GarchConfig, fit_garch11};

#[test]
fn test_empty_returns_rejected() {
    let res = fit_garch11(&[], &GarchConfig::default());
    assert_eq!(res, Err(AnalyticsError::EmptyInput));
}

#[test]
fn test_insufficient_data_rejected() {
    let res = fit_garch11(&[0.01, -0.01, 0.02], &GarchConfig::default());
    assert_eq!(
        res,
        Err(AnalyticsError::InsufficientData { needed: 10, got: 3 })
    );
}

#[test]
fn test_zero_variance_rejected() {
    let res = fit_garch11(&[0.01; 20], &GarchConfig::default());
    assert_eq!(res, Err(AnalyticsError::ZeroVariance));
}

#[test]
fn test_garch11_fitting_synthetic_clustering() {
    // Synthetic time series with clear volatility clustering
    let mut returns = Vec::new();
    for i in 0..100 {
        let scale = if (30..60).contains(&i) { 0.03 } else { 0.008 };
        let sign = if i % 2 == 0 { 1.0 } else { -1.0 };
        returns.push(sign * scale);
    }

    let config = GarchConfig {
        p: 1,
        q: 1,
        annualization: 252.0,
        forecast_horizon: 5,
    };

    let result = fit_garch11(&returns, &config).expect("garch fit succeeded");
    assert!(result.alpha > 0.0);
    assert!(result.beta > 0.0);
    assert!(result.persistence > 0.0 && result.persistence < 1.0);
    assert!(result.omega > 0.0);
    assert!(result.unconditional_variance > 0.0);
    assert!(result.half_life.is_some());
    assert_eq!(result.forecast_volatilities.len(), 5);
    assert_eq!(result.annualized_forecast_volatilities.len(), 5);
    assert_eq!(result.conditional_volatilities.len(), returns.len());
}
