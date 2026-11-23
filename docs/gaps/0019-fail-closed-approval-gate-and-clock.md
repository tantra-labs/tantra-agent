# 0019 — Job scheduler defaults fail open (`PermissiveApprovalGate`) and reads the wall clock

**Status**: open
**Filed**: 2026-11-23 — workspace design review (root plan 0021) · **Kind**: design-flaw · **Severity**: medium
**Plan**: [0021](../plans/0021-design-review-2026-11.md)

## What's stubbed or hardcoded

- `crates/tantra-agent-core/src/jobs/scheduler.rs:56-68`: `PermissiveApprovalGate` returns `Ok(true)` / `true` for everything. `:89` declares `pub struct JobScheduler<G = PermissiveApprovalGate>`, making it the default type parameter, and `JobScheduler::permissive()` (`:96`) uses the real `Utc::now`. `ManualApprovalGate` ("Safe default for production") exists but is not the default.
- `crates/tantra-agent-core/src/jobs/spec.rs:95`, `sqlite_store.rs:235`, `memory_store.rs:93`, `ref_store.rs:59` and `crates/tantra-agent-core/src/memory/mod.rs:101` call `Utc::now()` inside the library. `tantra-agent-core` is an L3 crate so this is not hard-banned, but jobs and ids are not replayable or testable without the injected `Clock`, which only the scheduler uses.

## Why

A caller who writes `JobScheduler::<_>::default()` or `permissive()` auto-approves model-created jobs, including `trade-intent` jobs queued by `FeedTradeIntentSink`, which relies on `ApprovalPolicy::Manual`. The default is fail-open, contrary to the fail-closed posture in `.agents/rules/security-and-secrets.md` §1 and the untrusted-AI principle.

## How to close it

1. Remove the default type parameter; rename the permissive gate to `AutoApproveGate` behind `#[cfg(any(test, feature = "testing"))]`.
2. Make `ManualApprovalGate` or the runtime approval queue the only production gates.
3. Thread `Clock` into `JobSpec::new`, the stores and `RefStore`.

**Verification**: test in `crates/tantra-agent-core/tests/` that a `trade-intent` job is never approved by a scheduler built from production constructors; deterministic-clock tests for job ids and stores; `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `make check`, targeted `cargo test -p tantra-agent-core --profile prod`, `make prod`.

## Related

- Root gap 0013 (risk/approval gating must be single and fail-closed).
- [0012](0012-propose-trade-untrusted-ai-boundary.md): the chat-side approval boundary.
