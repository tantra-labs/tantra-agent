# 0011 — MCP `run_backtest` / `optimize_strategy` / `explain_decision` fabricate results

**Status**: open
**Filed**: 2026-11-23 — workspace design review (root plan 0021) · **Kind**: design-flaw · **Severity**: high
**Plan**: [0021](../plans/0021-design-review-2026-11.md)

## What's stubbed or hardcoded

- `crates/tantra-agent-mcp/src/tools.rs:1225-1250` `BacktestDispatcher::dispatch` mints a random UUID and returns `"status": "queued"` (`:1249`), `"statutory_costs_enforced": true`, `"settlement_timing": "T+1"` and a "Backtest dispatched to honba-ya actor runtime" message. Nothing is dispatched anywhere.
- `crates/tantra-agent-mcp/src/tools.rs:1460-1495` `optimize` returns `"status": "started"` (`:1490`) with `job_id = format!("sweep_{algorithm}_{trials}_{seed}")`. The id is deterministic and no job exists.
- `crates/tantra-agent-mcp/src/tools.rs:1514-1580` `explain` builds "ALL PASSED / REFUSED" text (`:1562`) purely from caller-supplied `risk_check_results[].passed` (`unwrap_or(false)`): the model grades its own risk check. `price` defaults to `0.0`.
- `crates/tantra-agent-mcp/src/server.rs:420-430` routes these tool names to the stubs. No `PortError::unavailable` is returned and no gap is cited.
- `qty`, `price`, `starting_cash` and `risk_free_rate` are parsed with `as_f64()` (`tools.rs:1236,1530,1540,1275`).

## Why

An LLM client is told a backtest or sweep is running, with "statutory costs enforced", when nothing runs; it then polls or reports results that do not exist. `explain_decision` can emit a regulator-looking "ALL PASSED" explanation for a decision that was never risk-checked. This breaks `.agents/rules/interface-first-and-modularity.md` §1.3 (explicit `PortError::unavailable`, never fake defaults), `.agents/rules/gap-tracking.md` §2 (uncited stub) and the zero-floats rule in §2.3 of the same interface rule.

## How to close it

1. Replace `BacktestDispatcher` and `optimize` with ports (`BacktestPort`, `OptimizerPort`), following the `StrategyLoopPort` pattern in `tantra-agent-core::strategy_loop`, backed by `RuntimeLibraryClient` against runtime `/api/v2` jobs. When unwired, return a JSON-RPC error / `unavailable` rather than a success payload.
2. Make `explain_decision` take an audit-record id and read the real `LlmAuditRecord` / `RiskRefusal` from the WAL via a port instead of echoing caller input.
3. Parse money and rates as `Decimal` strings; drop `as_f64()` on these paths.
4. Remove the hardcoded `statutory_costs_enforced` / `settlement_timing` claims unless they come from the runtime response.

**Verification**: contract test in `crates/tantra-agent-mcp/tests/` that an unwired server returns a JSON-RPC error for the three tools; golden vector for `explain_decision` against a recorded refusal; `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `make check`, targeted `cargo test -p tantra-agent-mcp --profile prod`, `make prod`.

## Related

- Root gap 0012 (fabricated or synthetic data presented as real).
- Root plan 0019 P0.7 only covers the `honba` wording in these messages, not the stub behaviour.
- [0021](../plans/0021-design-review-2026-11.md) R1 (tool-dispatch duplication) touches the same file.
