# Gap Index — `tantra-agent`

Per workspace rule `.agents/rules/gap-tracking.md`:
- **Gaps** (`docs/gaps/XXXX-*.md`): individual missing features, stubs, or rule violations.
- **Plans** (`docs/plans/XXXX-*.md`): collections of gaps.
- Numbering is per-repo, sequential, never reused.

## Open Gaps

| # | Title | Severity | Status | Plan |
|---|-------|----------|--------|------|
| [0003](0003-persistent-memory-not-implemented.md) | `UserMemory` persistent memory with profile budgeters not implemented | Medium | Open | [0009](../plans/0009-persistent-memory-mcp-client-and-phase-e.md) |
| [0004](0004-mcp-stdio-client-bridge.md) | MCP stdio client bridge not implemented in `tantra-agent-mcp` | Medium | Open | [0009](../plans/0009-persistent-memory-mcp-client-and-phase-e.md), [0007 Phase 17](../plans/0007-comprehensive-mcp-ecosystem-and-agent-implementation-guide.md) |
| [0005](0005-research-note-not-implemented.md) | `ResearchNote` in `tantra-agent-report` not implemented | Low | Open | [0009](../plans/0009-persistent-memory-mcp-client-and-phase-e.md) |
| [0006](0006-monitor-scheduler-bridge.md) | Runtime-agent monitor scheduler bridge not implemented (agent-side job scheduler exists) | Low | Open | [0009](../plans/0009-persistent-memory-mcp-client-and-phase-e.md) |
| [0007](0007-prompt-cache-regression-tests.md) | Cache-hit regression tests for prompt caching not implemented | Low | Open | [0009](../plans/0009-persistent-memory-mcp-client-and-phase-e.md) |

## Closed Gaps

| # | Title | Closed In |
|---|-------|-----------|
| 0001 | Inline `#[cfg(test)]` blocks in `tantra-agent-core/src/ref_store.rs` + `tantra-agent-research/src/verify/mod.rs` | Moved to `tests/ref_store_tests.rs` (new) + `tests/verify_tests.rs` (existing coverage) 2026-11-13 (plan 0008); 4 + 10 tests green |
| 0002 | `tantra-agent-chat-tools` crate (read-only tools + `PureToolAdapter`) | Implemented (Phase B) in commit `f3f1cdc` (read-only tools & ports) + `85d3c07` (strategy advisory tools), expanded by plan 0006 Phases 1–4 (`a72689c`, `98c66c2`, `127597d`). See `docs/design/code-chat-tools.md`. |
