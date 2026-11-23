# 0015 — `OpenAiCompatibleProvider` derives `Debug` over a plaintext API key; SSE parser buffers are unbounded

**Status**: open
**Filed**: 2026-11-23 — workspace design review (root plan 0021) · **Kind**: design-flaw · **Severity**: medium
**Plan**: [0021](../plans/0021-design-review-2026-11.md)

## What's stubbed or hardcoded

- `crates/tantra-agent-llm/src/openai_compat.rs:93-103`: `#[derive(Debug, Clone)] struct OpenAiCompatibleProvider { api_key: Option<String>, ... }`. `{:?}` or `tracing` of the provider (or `ConfigCatalog`) prints the bearer key. The config layer redacts (`providers.rs:97-120,180-186`) but the resolved provider does not. `api_key` is a plain `String` (no `Zeroize`) and is cloned once per model (`providers.rs:428`).
- `crates/tantra-agent-llm/src/openai_compat.rs:347-395`: `line_buf`, `text` and `ToolAccum.arguments` grow without limit; a misbehaving or malicious `base_url` endpoint can stream gigabytes (no `max_response_bytes`). The non-stream path `response.json()` (`:253`) is also uncapped.
- `crates/tantra-agent-llm/src/openai_compat.rs:283`: error body `unwrap_or(Value::Null)` loses the provider message for 5xx; `LlmError::Transport(e.to_string())` may include the URL.
- No per-request size cap on `build_chat_payload` (`:200-215`).

## Why

Secrets in logs violate `.agents/rules/security-and-secrets.md` §3.3. An untrusted OpenAI-compatible endpoint (OmniRoute and free-tier gateways are used per plan 0004) can exhaust host memory.

## How to close it

1. Implement a manual `Debug` printing `[REDACTED]`; store the key as `secrecy::SecretString` or `Zeroizing<String>`.
2. Share one `reqwest::Client` per provider block instead of per model.
3. Add `MAX_STREAM_BYTES`, `MAX_TOOL_ARG_BYTES` and `max_request_bytes`; return a typed `LlmError::ResponseTooLarge`.
4. Keep the provider error body for 5xx and strip the URL from transport errors.

**Verification**: test in `crates/tantra-agent-llm/tests/` that `format!("{:?}", provider)` contains no key; wiremock test that an oversize stream returns `LlmError::ResponseTooLarge`; `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `make check`, targeted `cargo test -p tantra-agent-llm --profile prod`, `make prod`.

## Related

- Root gap 0011 (credentials/tokens leak via `Debug`, logs and URLs).
- [0013](0013-llm-context-budget-retries-cost.md) edits the same provider (retry/fallback wrapping).
