# 0017 — ACP host: sessions never freed, in-memory store, no caller scoping, divergent system prompt

**Status**: open
**Filed**: 2026-11-23 — workspace design review (root plan 0021) · **Kind**: design-flaw · **Severity**: medium
**Plan**: [0021](../plans/0021-design-review-2026-11.md)

## What's stubbed or hardcoded

- `crates/tantra-agent-acp/src/agent.rs:42,163-185`: sessions live in `Arc<Mutex<HashMap>>`. `max_sessions` is enforced on `session/new`, but there is no `.remove`, no close and no idle TTL. After 16 (`crates/tantra-agent-acp/src/bin/tantra-acp.rs:65`) `session/new` calls the host refuses everyone until restart.
- `crates/tantra-agent-acp/src/bin/tantra-acp.rs:60-66`: `InMemorySessionStore`, a one-line hard-coded `SYSTEM_PROMPT` (not the `tantra-agent-prompt` personas) and `create_strategy_tools(None)` (stub runner). History lives in `SessionState` and is lost on restart.
- `crates/tantra-agent-acp/src/agent.rs:188-206`: no authenticated caller; any peer on the connection can drive any session id. `grants` (`allow_always`) are persisted in the session map without audit.
- `tantra-runtime/crates/tantra-runtime-api-rest/Cargo.toml:27` embeds this host, so the same limits apply server-side.

## Why

A slot leak is a trivial denial of service. In-memory history is not restart-safe or horizontally scalable, violating `.agents/rules/scalability-and-concurrency.md` §4 for any hosted use.

## How to close it

1. Handle `session/close`, add an idle-TTL sweeper task and a `Drop` guard on connection end.
2. Give `AgentHost` a `SessionStore` backed by runtime persistence (`tantra-runtime-persistence/src/sessions.rs` already exists), keyed by `(caller_id, session_id)`.
3. Build the system prompt from `tantra-agent-prompt`.
4. Audit `allow_always` grants (who granted which tool, when).

**Verification**: test in `crates/tantra-agent-acp/tests/` with a fake client that opens 17 sessions, closing some, and asserts slots are reclaimed; cross-caller session access is rejected; `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `make check`, targeted `cargo test -p tantra-agent-acp --profile prod`, `make prod`.

## Related

- Plan [0019](../plans/0019-unified-acp-agent-protocol.md) covers the protocol, not session lifecycle.
- [0012](0012-propose-trade-untrusted-ai-boundary.md): grant semantics for `allow_always`.
- [0013](0013-llm-context-budget-retries-cost.md): history growth.
