# 0013 — No total context budget or compaction, `context_window()` unused, no retries or cost cap

**Status**: open
**Filed**: 2026-11-23 — workspace design review (root plan 0021) · **Kind**: design-flaw · **Severity**: high
**Plan**: [0021](../plans/0021-design-review-2026-11.md)

## What's stubbed or hardcoded

- `crates/tantra-agent-chat/src/turn.rs:222-262` rebuilds `ChatRequest` from the entire `history` every round (max_rounds 24, `turn.rs:113-125`). `compacted: false` is hardcoded (`turn.rs:181,198,443,500,899`); nothing compacts.
- `crates/tantra-agent-llm/src/lib.rs:436` declares `fn context_window(&self)`; `crates/tantra-agent-llm/src/openai_compat.rs:319-321` returns constant `128_000` and ignores `ModelInfo.context` parsed from `tantra.jsonc` (`providers.rs:435`). No caller of `context_window()` exists outside tests.
- The only size cap is `MAX_TOOL_OUTPUT_BYTES = 64 KiB` per tool (`turn.rs:30,797-801`). 24 rounds times N parallel calls times 64 KiB is unbounded relative to an 8k-32k local model.
- `crates/tantra-agent-llm/src/openai_compat.rs:162-173,649-661`: single attempt, no retry or backoff; `429` maps to `LlmError::RateLimited` without `Retry-After`. No provider fallback chain (`ConfigCatalog::build`, `providers.rs:405-450`, picks one model).
- `Usage` is collected (`turn.rs:150,387`) but there is no per-session or per-tenant token or cost cap.

## Why

A long chat, or a tool returning 64 KiB several times, overflows the model context: a hard HTTP 400 mid-turn or silent provider-side truncation. Transient 429/5xx responses abort the whole turn. A runaway 24-round loop has no cost ceiling.

## How to close it

1. Add `ContextBudget` in `tantra-agent-chat` using `ModelInfo.context` / `LlmProvider::context_window` (make the provider return the configured value). Estimate tokens and drop or summarise the oldest tool results, replacing them with a `ref_store` handle (`tantra-agent-core/src/ref_store.rs` exists).
2. Add a retry policy (exponential backoff with jitter, honour `Retry-After`, only for idempotent pre-first-token failures) as a `RetryingProvider<P>` decorator, plus a `FallbackProvider` driven by a `fallback: [..]` list in `tantra.jsonc`.
3. Add `TurnConfig.max_total_tokens` and a per-session cost cap fed by `Usage`.

**Verification**: tests with a mock provider in `crates/tantra-agent-chat/tests/` and `crates/tantra-agent-llm/tests/`: budget exceeded, 429 then success, fallback to second provider; `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `make check`, targeted `cargo test -p tantra-agent-chat --profile prod`, `make prod`.

## Related

- [0015](0015-llm-provider-secret-and-buffers.md): same provider file (`openai_compat.rs`); share one `reqwest::Client` change.
- [0017](0017-acp-session-lifecycle.md): session lifecycle bounds history growth at the host level.
