# Gap Index — `tantra-agent`

Per workspace rule `.agents/rules/gap-tracking.md`:
- **Gaps** (`docs/gaps/XXXX-*.md`): individual missing features, stubs, or rule violations.
- **Plans** (`docs/plans/XXXX-*.md`): collections of gaps.
- Numbering is per-repo, sequential, never reused.

## Open Gaps

None — all filed gaps are implemented (see Closed below).

## Closed Gaps

| # | Title | Closed In |
|---|-------|-----------|
| 0001 | Inline `#[cfg(test)]` blocks in `tantra-agent-core/src/ref_store.rs` + `tantra-agent-research/src/verify/mod.rs` | Moved to `tests/ref_store_tests.rs` (new) + `tests/verify_tests.rs` (existing coverage) 2026-11-13 (plan 0008); 4 + 10 tests green |
| 0002 | `tantra-agent-chat-tools` crate (read-only tools + `PureToolAdapter`) | Implemented (Phase B) in commit `f3f1cdc` (read-only tools & ports) + `85d3c07` (strategy advisory tools), expanded by plan 0006 Phases 1–4 (`a72689c`, `98c66c2`, `127597d`). See `docs/design/code-chat-tools.md`. |
| 0003 | `UserMemory` persistent memory with profile budgeters not implemented | plan 0009 Phase C 2026-11-14 — `tantra-agent::memory::user::UserMemory` (per-profile `ProfileBudget`s, recall renderers via `tantra-agent-prompt`), commit `7f9d480` |
| 0004 | MCP stdio client bridge not implemented in `tantra-agent-mcp` | plan 0009 Phase E / 0007 Ph 17 2026-11-14 — `tantra-agent-mcp::client::{McpClient, McpClientError}` + `mcp_stdio_server` bin (process-spawned tests), commit `7f9d480` |
| 0005 | `ResearchNote` in `tantra-agent-report` not implemented | plan 0009 Phase E 2026-11-14 — `tantra-agent-report::notes::{ResearchNote, NoteVerdict}` + Typst/Markdown rendering + `tantra-agent-research::notes::research_note_from_verification`, commit `7f9d480` |
| 0006 | Runtime-agent monitor scheduler bridge not implemented | plan 0009 Phase D 2026-11-14 — `tantra-runtime-persistence::monitor::MonitorSchedulerBridge` over the agent `JobScheduler`/`JobStore` ports (6 tests), commit `634de2c` |
| 0007 | Cache-hit regression tests for prompt caching not implemented | plan 0009 Phase C 2026-11-14 — `tests/prompt_cache_tests.rs` (agent-core) + `tests/renderer_cache_tests.rs` (agent-prompt), commit `7f9d480` |
| 0008 | Track 4 regime/changepoint/copula/permutation engines not implemented | plan 0007 Track 4 Ph 9–10 2026-11-14 — `tantra-agent-analytics::{regime/hmm, changepoint, copula, validation::permutation_*}` + MCP tools `tantra_detect_regimes`/`tantra_change_point`/`tantra_copula_tail_risk`/`tantra_permutation_test`, commit `95ec800` |
