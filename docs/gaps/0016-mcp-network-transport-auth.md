# 0016 — MCP HTTP/SSE transport: no authentication, session leak, `/rpc` bypasses sessions, single static runtime token

**Status**: open
**Filed**: 2026-11-23 — workspace design review (root plan 0021) · **Kind**: design-flaw · **Severity**: medium
**Plan**: [0021](../plans/0021-design-review-2026-11.md)

## What's stubbed or hardcoded

- `crates/tantra-agent-mcp/src/transport.rs:138-149`: `create_router` mounts `/sse`, `/message` and `/rpc` with no auth middleware, no `Caller`, no CORS/Origin check and no body-limit layer (axum default only).
- `crates/tantra-agent-mcp/src/transport.rs:104`: `unregister` is never called (only its definition exists). Disconnected SSE clients stay in `SseSessionManager.sessions` forever, so the map grows without bound.
- `crates/tantra-agent-mcp/src/transport.rs:182-200`: `/message` answers on both the SSE stream and the HTTP body; any caller who obtains a `sessionId` can inject messages into another session. The id is the only capability and travels in the URL query.
- `crates/tantra-agent-mcp/src/library_http.rs:77-130`: all tools proxy to the runtime with one process-wide `TANTRA_RUNTIME_TOKEN`; no per-caller identity is forwarded, so runtime-side `Caller` scoping is lost and every MCP client acts as the same principal.
- `crates/tantra-agent-mcp/src/bin/mcp_stdio_server.rs` is stdio-only; `create_router` is used only by tests (`crates/tantra-agent-mcp/tests/unit_tests.rs:472`), so the network transport is untested for auth and unreachable from any binary.

## Why

Once the router is bound on a non-loopback interface (plan 0007 Ph16 intends this) every tool, including runtime-proxied study and loop calls, is world-callable. Violates `.agents/rules/security-and-secrets.md` §1.1-1.2 (fail-closed auth) and §2.1 (`Caller` scoping).

## How to close it

1. Add an `McpAuth` trait (bearer/JWT using the same validators as runtime config) mounted fail-closed, refusing a non-loopback bind without auth.
2. Derive `session_id` server-side, bind it to the authenticated caller, and reject mismatches.
3. Call `unregister` from a drop guard on stream end.
4. Forward the caller token to `RuntimeLibraryClient` per request (`with_token`) instead of the process-wide token.
5. Add `DefaultBodyLimit` and an Origin check.

**Verification**: tests in `crates/tantra-agent-mcp/tests/`: unauthenticated request returns 401, cross-session POST returns 403, session count returns to zero after stream drop; `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `make check`, targeted `cargo test -p tantra-agent-mcp --profile prod`, `make prod`.

## Related

- Root plan 0019 P1.5 covers only the unbounded channel type in this transport.
- Deployment note: until this closes, the SSE transport must stay behind loopback or stdio.
- [0011](0011-mcp-stub-tools-fabricate-results.md) is in the same crate.
