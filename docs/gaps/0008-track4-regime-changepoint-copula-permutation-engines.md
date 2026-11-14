# Gap 0008 — Track 4 Advanced Statistical & Risk Engines Not Implemented

Severity: Medium · Status: Open · Tracked by: plan `0007` Track 4 Phases 9–10

## What's Missing

`tantra-agent-analytics` has no regime-switching HMM, change-point detection,
copula tail-risk, or permutation-test modules (only a `regime/mod.rs` stub), and
`tantra-agent-mcp` exposes no corresponding gateway tools. Plan 0007 Track 4
Phases 9–10 scope:

1. `regime/hmm.rs` — 2/3-state Gaussian Hidden Markov Model (Baum-Welch training,
   Viterbi state decoding) classifying `BullishTrend` / `BearishTrend` /
   `ChoppyVolatile`.
2. `changepoint.rs` — CUSUM structural-break detection with break timestamps.
3. `copula.rs` — bivariate Clayton/Gumbel copula fitting, tail dependence
   coefficients, joint VaR.
4. `validation.rs` — permutation testing: shuffle trade returns N times to get
   the empirical p-value of observed Sharpe / win rate.
5. MCP tools `tantra_detect_regimes`, `tantra_change_point`,
   `tantra_copula_tail_risk`, `tantra_permutation_test` registered in
   `tantra-agent-mcp` (schema + dispatch + envelope).

## Constraints

- Analytics internals may be `f64` (established `tantra-agent-analytics`
  pattern); deterministic seeded RNG (`SplitMix64`) for any resampling.
- Tests in `crates/tantra-agent-analytics/tests/` and
  `crates/tantra-agent-mcp/tests/` (no inline `#[test]`).
- MCP outputs wrapped in `TantraResultEnvelope`.
