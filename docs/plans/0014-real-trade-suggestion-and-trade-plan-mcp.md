# 0014 — Real Trade Suggestion & `trade_plan` MCP Tools

**Status**: Proposed

## 0. Executive Summary & Objective

Replace the stubbed `strategy_suggest` output with a typed `TradePlan` via a `TradePlanPort`, and add MCP tools `trade_plan` and `trade_plan_presets`. Roadmap: [root plan 0008](../../../docs/plans/0008-trade-signal-generator.md).

## 1. Context & Gaps Covered

- `crates/tantra-agent-chat-tools/src/strategy.rs` `TradeSuggestionTool` (`strategy_suggest`, `TradeSuggestionInput{strategy, symbol, capital?, risk_pct?}`) calls
  `ports/strategy.rs` `DefaultStrategyRunner::generate_trade_suggestion(...) -> Result<Value, String>`, a **stub**: hardcoded entries (NIFTY50 24150, RELIANCE 2980, else 1000),
  SL = entry x 0.988 though described as ATR, always BUY, fixed 2R/3R targets, untyped JSON, stringly-typed error.
- MCP: `crates/tantra-agent-mcp/src/tools.rs` (`list_tool_definitions`, `json!`) and `server.rs` `call_tool` match.
- Sibling: plan [0013](0013-screener-mcp-tools.md). Upstream: `tantra-core/docs/plans/0005-trade-planner-and-manager.md`; wiring: `tantra-runtime/docs/plans/0006-trade-manager-backtest-and-synthetic-brackets.md`.

## 2. Component Specifications & Phases

- **P1 Port + DTOs**: `TradePlanPort { plan(request) -> Result<TradePlan, PortError>; presets() -> Result<Vec<PresetInfo>, PortError> }`, request/response using core types (`TradePlan`, `TradePlanSpec`, `RiskBudget`). Default impl returns `PortError::unavailable(...)` (citing this plan) — never fake data.
- **P2 Chat tool**: `strategy_suggest` returns a typed `TradePlan`; remove hardcoded prices and the string error; stub deleted once the port is wired.
- **P3 MCP**: `trade_plan` (inputs: `symbol`, `preset`, `overrides`, `capital`, `risk_pct`) and `trade_plan_presets`; register in `tools.rs` + `call_tool`; ReadOnly (plans only, no order placement); bounded payloads.
- **P4 Wiring**: concrete port implemented in `tantra-runtime` (plan 0006 P5) at the composition root.
- **Untrusted AI**: an LLM-proposed preset/overrides are validated into closed core types (`deny_unknown_fields`); any resulting order still passes `RiskEngine`. Tools never bypass risk.
- Tier rules: R1 (no runtime/exec/market deps), R4 (market-data wiring stays in runtime; only `tantra-agent-chat-tools` may depend on `tantra-ledger-*`); `tantra-agent-tools` stays pure.

## 3. Test Plan (TDD)

`tests/` only: unavailable-port returns `PortError::unavailable` (not empty data); mock port golden plan through chat tool and MCP; override-validation negatives (unknown field, float price);
tool-catalogue count pin bump; `dependency_rules.rs` stays green.

## 4. Dependencies & Tier Notes

Needs core plan 0005 types. Until then P1 may only define the port against local DTOs flagged with a citation to core plan 0005.

## 5. Open Questions

- Should `trade_plan` accept a free-form signal, or only symbol + preset (default)?
