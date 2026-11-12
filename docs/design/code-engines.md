# `tantra-agent` — Analytics, Genetic, Forecast, Panel & Report Reference

Documents `tantra-agent-analytics/`, `tantra-agent-genetic/`, `tantra-agent-forecast/`,
`tantra-agent-panel/`, and `tantra-agent-report/`.

## `tantra-agent-analytics`

Performance attribution, Deflated Sharpe Ratio (DSR), Probability of Backtest Overfitting
(PBO), walk-forward validation, holdout, Monte Carlo, cost stress, round-trip & trade stats,
and equity-curve analytics. Public modules (`src/lib.rs`): `cost_stress`, `dsr`,
`equity_stats`, `error`, `factor_attribution`, `holdout`, `math`, `monte_carlo`, `pbo`,
`report` (`PerformanceReport::from_returns` / `from_equity_curve`), `round_trip`,
`trade_stats`, `validation`, `walk_forward`.

> **Orphaned source files**: `src/cointegration.rs` and `src/tearsheet.rs` exist on disk but
> are **not declared in `lib.rs`** and do not compile. `cointegration` is the target of plan
> 0007 Phase 1 (wire it up and expose as an MCP tool); `tearsheet`'s functionality lives in
> `tantra-agent-report` instead.

Key engines:
- **DSR** — Deflated Sharpe Ratio (multiple-testing adjustment for Sharpe).
- **PBO** — Probability of Backtest Overfitting.
- **Walk-forward** — out-of-sample validation (`walk_forward`).
- **Holdout / validation** — sample-split and permutation validation.

## `tantra-agent-genetic`

StockSharp Odysseus-style genetic programming & rule-tree strategy synthesizer &
parameter mutator. Modules: `ast` (rule-tree AST), `genetic` (GA engine), `regime`,
`synthesizer`.

## `tantra-agent-forecast`

Deterministic time-series forecasting. Modules: `arima`, `ets`, `features`,
`gradient_boost`, `linalg`, `tbats`, `theta`. Backs the MCP `tantra_forecast_series` tool.

## `tantra-agent-panel`

Econometric panel models. Modules: `asset_pricing`, `fama_macbeth` (2-pass regression),
`fixed_effects`, `gmm`, `matrix`, `random_effects`.

## `tantra-agent-report`

Embedded Typst / Markdown quantitative research tearsheet & PDF report generator consuming
`tantra-core` `Decimal` metrics. Public types: `TearsheetReport`, `Metric`, `ReportSection`;
LLM narrative narration (`narrate`, `llm` feature). Modules: `lib`, `narrate`.

### Note (gap 0005)

`ResearchNote` (structured research note: hypothesis, evidence refs, verification outcome,
metrics) is planned but not implemented
(`docs/gaps/0005-research-note-not-implemented.md`, tracked by plan 0009).

## Zero-Floats Policy

All financial figures, quantities, prices, metrics, and drawdown numbers strictly use
`rust_decimal::Decimal` — no `f32`/`f64` in analytics/forecast/panel/report paths.

## Related

- `tantra-agent-core::result` — `TantraResultEnvelope` wraps tool outputs.
- `tantra-agent-core::indicators` — `IndicatorCatalog` (approved indicators vocabulary).
