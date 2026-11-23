# 0021 — tantra-agent design review (2026-11): untrusted-AI boundary, honest tools, context budgets & MCP/ACP hardening

**Status**: Proposed (2026-11-23)
**Scope**: `tantra-agent` · **Umbrella**: root `docs/plans/0021-workspace-design-review-2026-11.md`

## 0. Executive Summary & Objective

The 2026-11 design review of `tantra-agent` found that the untrusted-AI boundary is weaker in the chat and MCP layers than the architecture documents claim. Three MCP tools report success for work that never happens (`crates/tantra-agent-mcp/src/tools.rs:1249` returns `"status": "queued"` for a backtest that is never run, `:1490` returns `"status": "started"` for an optimisation job that does not exist, `:1562` emits `"ALL PASSED"` from caller-supplied check results). `propose_trade` lets the model choose the SEBI algo id, and `PermissionDecision::AllowForSession` (`crates/tantra-agent-chat/src/turn.rs:665-668`) inserts the tool NAME into `session_grants`, so one approval covers every later `propose_trade` call regardless of instrument or quantity.

Beyond that: no context budget, retry or cost cap; tool output reaches the model with no trust boundary; a provider that derives `Debug` over its API key; an unauthenticated MCP network transport; ACP sessions that are never freed; Python/Typst codegen by string interpolation; and a scheduler whose default approval gate approves everything.

This plan closes those gaps in three phases and schedules three plan-only items (R1-R3). Non-goals: the MCP `tools.rs` file split and analytics relocation (root plan 0019 P3.1 / P4.2), the unbounded SSE channel type (0019 P1.5), the unified ACP protocol work (plan [0019](0019-unified-acp-agent-protocol.md)), and the `honba` wording clean-up (0019 P0.7).

## 1. Context & Gaps Covered

| Gap | Title | Kind | Severity |
|---|---|---|---|
| [0011](../gaps/0011-mcp-stub-tools-fabricate-results.md) | MCP stub tools fabricate results | design-flaw | high |
| [0012](../gaps/0012-propose-trade-untrusted-ai-boundary.md) | `propose_trade` untrusted-AI boundary | design-flaw | high |
| [0013](../gaps/0013-llm-context-budget-retries-cost.md) | Context budget, retries and cost cap | design-flaw | high |
| [0014](../gaps/0014-tool-result-trust-boundary.md) | Tool-result trust boundary | design-flaw | high |
| [0015](../gaps/0015-llm-provider-secret-and-buffers.md) | Provider secret redaction and bounded buffers | design-flaw | medium |
| [0016](../gaps/0016-mcp-network-transport-auth.md) | MCP network transport auth | design-flaw | medium |
| [0017](../gaps/0017-acp-session-lifecycle.md) | ACP session lifecycle | design-flaw | medium |
| [0018](../gaps/0018-codegen-interpolation-safety.md) | Codegen interpolation safety | design-flaw | medium |
| [0019](../gaps/0019-fail-closed-approval-gate-and-clock.md) | Fail-closed approval gate and clock | design-flaw | medium |

Plan-only items (no standalone gap doc):

- **R1 — Make the R1-R4 dependency test as strong as the written rules.** `crates/tantra-agent-chat/tests/dependency_rules.rs` checks only direct `--no-deps` manifests, not the resolved graph; R1 hard-codes two exceptions (`mcp -> tantra-market-pricing`, `mcp -> tantra-ledger-allocation`) that `.agents/rules/architecture-and-coupling.md` §2.2 does not state; R3 checks only `tokio|async-trait|reqwest|sqlx`; upward edges (`tantra-ffi`, `tantra-ui*`) and R5 are not asserted; `tantra-runtime-api` depends on `tantra-agent-chat-tools` (`tantra-runtime/crates/tantra-runtime-api/Cargo.toml:16`), a same-tier edge nobody tests.
- **R2 — One tool contract for chat and MCP.** `crates/tantra-agent-mcp/src/tools.rs` hand-rolls JSON argument parsing (48 `f64` uses) and hand-written schemas (`tools.rs:831-946`), while chat tools use typed `schemars` inputs (`crates/tantra-agent-chat-tools/src/trade_intent.rs:72-90`); `McpServer::call_tool` (`server.rs:396-500`) is a second dispatch path. `MetricsCalculatorService` (`tools.rs:1275-1400`) duplicates `tantra-core/src/quant.rs:360` and `tantra-agent-analytics::{equity_stats,metrics,dsr,pbo}` in `f64`, so chat and MCP answers diverge (different annualisation default 6% rf).
- **R3 — Honest permission model and plugin surface.** `crates/tantra-agent-plugins/src/lib.rs` is a manifest registry with no loader, sandbox or `ToolRisk` mapping; `crates/tantra-agent-mcp/src/frontier.rs:153-165` writes client configs to disk from a library crate; `ToolTraits::default()` (`crates/tantra-agent-chat/src/permission.rs:50-60`) is `ReadOnly`, so a tool that forgets `traits()` is silently read-only; `ToolRisk::External` / `Execute` (`permission.rs:25-29`) are unused, and strategy-loop eval/sweep tools that consume runtime resources are classed `ReadOnly`.

## 2. Component Specifications & Phases

### Phase A — Quick, low-risk fixes
1. **0011** — return `unavailable` from the three stub tools until ports exist; remove hardcoded claims; parse money as `Decimal` strings.
2. **0015** — manual redacted `Debug`, `SecretString` key, response/request byte caps.
3. **0019** — remove the default `PermissiveApprovalGate`, gate it behind `testing`.
4. **0014 (part)** — return `ToolError::InvalidArguments` for unparsable arguments; JSON-structure truncation.
5. **0012 (part)** — add `ToolRisk::Trade`, exclude it from session grants; cap `rationale` length.

**Gate**: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `make check`, `cargo test -p tantra-agent-chat --test unit_tests --profile prod`, `cargo test -p tantra-agent-llm --profile prod`, `cargo test -p tantra-agent-mcp --profile prod`, `make prod`.

### Phase B — Structural refactors
1. **0012** — typed `TradeIntent` over `tantra_core::Decision` (provider: `tantra-core`), algo id injected by the runtime sink, sink dry-runs `RiskEngine::check`, per-caller pending cap.
2. **0014** — typed result envelope with `trust` field, taint tracking across a turn, injection-corpus golden transcripts.
3. **0013** — `ContextBudget`, `RetryingProvider<P>`, `FallbackProvider`, `TurnConfig.max_total_tokens`.
4. **0011** — `BacktestPort` / `OptimizerPort` backed by `RuntimeLibraryClient`; `explain_decision` reads the audit record by id.
5. **0018** — validated identifier newtypes or manifest output instead of Python codegen; `typst_escape()`; AST-based verifier.
6. **R2** — single `AgentTool` trait in `tantra-agent-core` adapted to `ChatTool` and `McpTool`; route `evaluate_metrics` to `tantra-agent-analytics`; chat-vs-MCP golden parity test.
7. **R3** — remove `Default` from `ToolTraits`, classify strategy-loop eval/sweep as `Execute`, define `PluginManifest.risk` before any loader, move `frontier.rs` behind an explicit CLI.
8. **R1** — resolved-graph dependency test, I/O deny-list for `tantra-agent-tools`, no `tantra-ffi|tantra-ui` deps, exceptions in one shared TOML table, mirrored test in `tantra-runtime`, update the rule document.

**Gate**: Phase A gate plus `cargo test -p tantra-agent-chat --test dependency_rules --profile prod`, `cargo test -p tantra-agent-genetic --profile prod`, `cargo test -p tantra-agent-report --profile prod`.

### Phase C — Deployment readiness
1. **0016** — `McpAuth`, server-derived session ids bound to the caller, drop-guard `unregister`, per-request token forwarding, body limit; refuse non-loopback bind without auth.
2. **0017** — `session/close`, idle-TTL sweeper, `SessionStore` over runtime persistence keyed by `(caller_id, session_id)`, shared system prompt.
3. Add readiness and metrics endpoints, per-turn/tool tracing spans and graceful shutdown/drain of in-flight turns (today only a static `/health` exists at `transport.rs:213`).
4. Durable audit of chat-turn tool calls and permission decisions (today only `SessionEvent`s to the UI channel); include template version in the replay key (`tantra-agent-core/src/prompt.rs:40` hashes prompt text only).

**Gate**: Phase B gate plus `cargo test -p tantra-agent-acp --profile prod`.

## 3. Relationship to Other Plans

- Not duplicated here: root `docs/plans/0019-codebase-audit-refactoring-unification-and-wiring.md` P0.7 (honba wording), P1.5 (SSE channel type), P3.1 (`tools.rs` split), P4.2 (analytics move), §4.3 (unlinked crates).
- [0019](0019-unified-acp-agent-protocol.md) (ACP) owns the protocol and web-to-ACP path; gap 0017 covers lifecycle only and should land before hosted use of that plan.
- Root gaps: 0011 (secrets in `Debug`), 0012 (fabricated data), 0013 (single fail-closed risk gate). Root plan 0022 (deployment architecture) receives the Phase C items; root plan 0021 is the umbrella.
- Provider-repo work: `tantra-core` shared `TradeIntent` type; `tantra-runtime` chat-trade sink and `/api/v2` job endpoints (runtime gap 0007 item 17 covers sink wiring only).

## 4. Exit Criteria

1. No MCP tool returns a success payload for work that was not performed; unwired ports return JSON-RPC errors.
2. A test proves `AllowForSession` never suppresses the prompt for `propose_trade`, and the model cannot supply an algo id.
3. A turn with oversized history stays within the model's configured context window; 429 then success and fallback are covered by tests.
4. Injection-corpus golden transcripts show untrusted tool output cannot trigger a mutating tool without re-confirmation.
5. `format!("{:?}")` of any provider or catalog contains no secret; oversize streams fail with a typed error.
6. Unauthenticated MCP network requests return 401; ACP sessions are reclaimed on close and idle TTL.
7. Generated Python and Typst from adversarial strings contain no extra statements or file reads.
8. No production constructor yields an auto-approving scheduler.
9. `dependency_rules.rs` passes on the resolved graph, with exceptions listed in one table shared with the rule document.
10. `make prod` green and this plan's gaps moved to the Closed table of `docs/gaps/README.md`.
