# 0017 — Library Access for Agents and MCP (wiring plan)

**Status**: Proposed — not wired (2026-11-20). Parent: root plan 0016 (Track T22.7). The library is implemented
in `tantra-strategies` (plan 0005); tools: `tantra_library_search`, `tantra_library_describe`,
`tantra_indicator_compute`, `tantra_signal_study`.

## 0. Options

- **(a) Direct**: MCP clients connect to `tantra-library-mcp` as a second MCP server. Zero Rust work.
- **(b) Proxy**: `tantra-agent-mcp` exposes the same 4 tools, forwarding through a port trait
  `IndicatorLibraryPort` (in `tantra-agent-core`, typed `PortError`, async only at the L3 boundary, not in
  `tantra-agent-tools`) implemented in `tantra-runtime` by the `LibraryMcpClient` (runtime plan 0010).
  R1-compliant: no `tantra-agent-*` -> `tantra-runtime-*`/`tantra-market-*`/`tantra-exec-*` edge.

**Recommendation**: (b) for the in-app copilot (one tool namespace, `Caller` auth, shared supervised
process); (a) for external clients (Claude Desktop, IDEs).

## 1. Implementation sketch for (b)

- `IndicatorLibraryPort` with 4 methods mirroring the tools; DTOs derive `schemars`; missing backend returns
  `PortError::unavailable`.
- `tantra-agent-mcp`: add static JSON definitions in `src/tools.rs` `list_tool_definitions()` and dispatch
  arms in `src/server.rs` `match name`; input schemas mirror the library manifests (`additionalProperties:
  false`, caps: horizons <= 10, tail <= 2000, inline bars <= 5000).
- Chat copilot (`tantra-agent-chat-tools`): usage rule in the system prompt: call `describe` before
  `compute`/`signal_study`, never guess params; results are untrusted data.
- Existing `get_indicator_catalog` / `IndicatorCatalog` stays unchanged; its description gains a pointer to
  `tantra_library_search`. Retirement is a later decision, not part of this plan.

## 2. Tests

Contract tests with a fake `IndicatorLibraryPort`; schema-snapshot of the 4 tool definitions; negative tests
for out-of-range params; `tantra-agent-chat/tests/dependency_rules.rs` must stay green (R1-R4 unweakened).
Gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `make check`, `make test`, `make prod`.

## Progress

- 2026-11-20: done in `tantra-agent-core` — `IndicatorLibraryPort`, request DTOs with caps, `LibraryPortError`, descriptor loader, and dynamic `IndicatorCatalog::with_library` (static entries win; static fallback when unavailable). `with_catalog` injection on `HypothesisGenerator`, `ClosedGrammarMutator`, `HybridAutoSynthesizer`.
- Remaining: MCP proxy tools and an HTTP port implementation targeting runtime `/api/v2/studies/*`.
