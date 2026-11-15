# 0014 — Real Trade Suggestion, `trade_plan`, & `chart_patterns` MCP Tools

**Status**: Proposed

## 0. Executive Summary & Objective

Replace the stubbed `strategy_suggest` output with a typed `TradePlan` via `TradePlanPort`, and add read-only MCP tools `trade_plan`, `trade_plan_presets`, and `chart_patterns` (backed by `PatternPort`).
Roadmap: [root plan 0008](../../../docs/plans/0008-trade-signal-generator.md).

## 1. Context & Gaps Covered

- `crates/tantra-agent-chat-tools/src/strategy.rs`: `TradeSuggestionTool` (`strategy_suggest`, `TradeSuggestionInput{strategy, symbol, capital?, risk_pct?}`) calls `ports/strategy.rs` `DefaultStrategyRunner::generate_trade_suggestion(...) -> Result<Value, String>`, a **stub**: hardcoded entries (NIFTY50 24150, RELIANCE 2980, else 1000), `SL = entry × 0.988` though described as ATR, always BUY, fixed 2R/3R targets, untyped JSON, stringly-typed error, and `TradeSuggestionTool::execute` emits `ToolOutput::ok("strategy_run", res)` instead of `"strategy_suggest"`.
- MCP: `crates/tantra-agent-mcp/src/tools.rs` (`list_tool_definitions`, `json!`) and `server.rs` `call_tool` match. Note: `crates/tantra-agent-mcp/Cargo.toml` carries legacy direct `tantra-ledger-allocation` / `tantra-market-pricing` dependencies against rule R4 — new tools must strictly use traits/ports (`TradePlanPort`, `PatternPort`) wired in `tantra-runtime`, never adding market/ledger deps to `tantra-agent-mcp`.
- Sibling: plan [0013](0013-screener-mcp-tools.md). Upstream: [`tantra-core` plan 0005](../../../tantra-core/docs/plans/0005-trade-planner-and-manager.md), [`tantra-market` plan 0002](../../../tantra-market/docs/plans/0002-chart-pattern-analyzer.md); wiring: [`tantra-runtime` plan 0006](../../../tantra-runtime/docs/plans/0006-trade-manager-backtest-and-synthetic-brackets.md).

## 2. Component Specifications & Phases

- **P1 Ports + DTOs**:
  - `TradePlanPort { plan(request) -> Result<TradePlan, PortError>; presets() -> Result<Vec<PresetInfo>, PortError> }` using core types (`TradePlan`, `TradePlanSpec`, `RiskBudget`, optional `ChartPattern` / preset selection).
  - `PatternPort { scan(PatternScanRequest { symbol, timeframe, lookback_bars?, include_forming? }) -> Result<Vec<ChartPattern>, PortError> }` returning core `ChartPattern` wire contracts (`kind`, `direction`, `stage`, `breakout_level`, `invalidation_level`, `measured_target`, `quality`, `pivots`).
  - Default impls return `PortError::unavailable(...)` (citing this plan) — never fake data.
- **P2 Chat tool**: `strategy_suggest` returns a typed `TradePlan`, fixes the `"strategy_run"` → `"strategy_suggest"` output/error tool-name bug, removes hardcoded prices and string errors, and deletes the stub once wired.
- **P3 MCP**:
  - `trade_plan` (inputs: `symbol`, `preset`, `overrides`, `capital`, `risk_pct`, optional `use_pattern: bool`)
  - `trade_plan_presets` (exposes canonical presets from `tantra-core` SSOT)
  - `chart_patterns` (inputs: `symbol`, `timeframe`, `lookback_bars?`, `include_forming?` → detected `ChartPattern` list with structural geometry)
  - Register in `tools.rs` + `call_tool`; `ToolRisk::ReadOnly` (plans & pattern geometry only, no order placement); bounded payloads (pivots capped/truncated cleanly if over response limit).
- **P4 Wiring**: Concrete `TradePlanPort` and `PatternPort` implemented in `tantra-runtime` (plan `0006` P5) at the composition root.
- **Untrusted AI**: LLM-proposed preset/overrides deserialize into closed core types (`#[serde(deny_unknown_fields)]`) and never bypass `RiskEngine` or `Decision` validation; SEBI `algo_id` remains mandatory on any downstream execution path.
- Tier rules: R1 (no runtime/exec/market deps), R3 (`tantra-agent-tools` stays pure), R4 (market-data wiring stays in `tantra-runtime`; no `tantra-market-*`/`tantra-ledger-*` additions in `tantra-agent-mcp`).

## 3. Test Plan (TDD)

`tests/` only:
- Unavailable-port returns `PortError::unavailable` (not empty data or hardcoded prices);
- Mock port golden `TradePlan` and `ChartPattern` through chat tool (`strategy_suggest` tool-name check) and MCP (`trade_plan`, `trade_plan_presets`, `chart_patterns`);
- Override-validation negatives (unknown field, float price);
- Tool-catalogue count pin bump; `dependency_rules.rs` stays green.

## 4. Dependencies & Tier Notes

Needs core plan `0005` (`TradePlan`, `ChartPattern`). Until then P1 may define ports against local DTOs flagged with a citation to core plan `0005`.

## 5. Open Questions

- Should `trade_plan` accept a free-form signal or only `symbol` + `preset` (+ optional `ChartPattern` scan/selection)? (Default: `symbol` + `preset`, with `use_pattern` optionally selecting the latest confirmed/armed pattern.)
