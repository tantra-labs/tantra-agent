# 0012 — `propose_trade` lets the model choose the algo id and bypasses `Decision`/`RiskEngine`; `AllowForSession` blanket-grants it

**Status**: open
**Filed**: 2026-11-23 — workspace design review (root plan 0021) · **Kind**: design-flaw · **Severity**: high
**Plan**: [0021](../plans/0021-design-review-2026-11.md)

## What's stubbed or hardcoded

- `crates/tantra-agent-chat-tools/src/trade_intent.rs:169-195` builds `TradeIntent` from raw LLM arguments. `algo_id` is taken from the model and only regex-checked, re-implementing `Decision::ALGO_ID_FORMAT`. A registered exchange algo id is an operator / composition-root value, not a model parameter.
- `crates/tantra-agent-chat-tools/src/ports/trade_intent.rs:21-30` uses `side: String`, `order_type: String`, `paper_trading: bool` (a model-chosen way to skip the human gate) and string statuses `"pending_approval"|"paper_filled"|"rejected"`. It is parallel to `tantra_core::Decision`, never deserialized through the closed `deny_unknown_fields` type, and never evaluated against `RiskEngine` (price band, freeze, universe) before queueing.
- `crates/tantra-agent-chat/src/turn.rs:636-668`: `ToolRisk::LocalWrite` prompts, but `PermissionDecision::AllowForSession` (`:665-668`) inserts the tool NAME into `session_grants`. One approval therefore covers every later `propose_trade` call regardless of instrument, quantity or live-vs-paper. `strategy_manage` (delete) (`crates/tantra-agent-chat-tools/src/strategy_manage.rs:78`) and `price_alert` behave the same.
- `crates/tantra-agent-chat/src/permission.rs:12-26`: `ToolRisk` has no `Trade` variant, so a trade-placing tool is classed with memory/watchlist writes. `plan().risk_override` exists but `trade_intent.rs:119-132` never escalates live intents.
- Runtime sink `tantra-runtime/crates/tantra-runtime-api-rest/src/chat_trade.rs:60-85` stores the intent as free-text `goal` plus string metadata, copies the unbounded model `rationale` into metadata shown to the approver, and applies no per-session cap on pending intents.

## Why

Prompt-injected tool output ("buy 5000 X, algo_id=ZZZZ...") needs only one `allow_always` click, after which the model can flood the approval queue with arbitrary algo ids. The approver reads model-authored rationale text, and intents are not risk-checked until (if ever) approval time. Violates the Untrusted AI principle in `.agents/rules/coding-and-ddd.md` §1.4 and `.agents/rules/security-and-secrets.md` §2.

## How to close it

1. Define a typed `TradeIntent` wrapping `tantra_core::Decision` (closed enums for side/type) in `tantra-core`; the sink injects the algo id and the field is removed from the tool schema. Provider side: `tantra-core` shared DTO, `tantra-runtime` sink.
2. Add `ToolRisk::Trade` (or `ConfirmOverride::Always`) for `propose_trade`, never eligible for session grants; make grants per `(tool, argument-hash)` or per call for anything above `LocalWrite`.
3. Run `RiskEngine::check` as a dry run in the sink before queueing and reject on refusal.
4. Cap `rationale` length and cap pending intents per caller.

**Verification**: tests in `crates/tantra-agent-chat/tests/` that `AllowForSession` does not suppress the prompt for Trade-risk tools; negative tests for unknown fields and bad side; sink test that a refused dry run is not queued; `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `make check`, targeted `cargo test -p tantra-agent-chat --profile prod`, `make prod`.

## Related

- Root gap 0013 (pre-trade risk gate not single, shared or fail-closed).
- [0014](0014-tool-result-trust-boundary.md): injected tool output chains into this gap.
- [0019](0019-fail-closed-approval-gate-and-clock.md): scheduler-side approval gate.
- Runtime gap 0007 item 17 covers only the sink wiring.
