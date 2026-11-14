# HANDOFF — tantra-agent

**Date**: 2026-11-14 · Repo: `tantra-agent` · Branch: `main`

## Phase / Slice Status

- **T6 track (2026-11-14) — plan 0009 Implemented, gaps 0003–0008 closed, zero open gaps**:
  - T6.1/2: `tantra-agent::memory::user::UserMemory` (per-profile `ProfileBudget` map, recall
    `profile_section`/`recall_section`/`assemble` via `tantra-agent-prompt` renderers) +
    prompt cache-hit/miss regression tests (`tests/prompt_cache_tests.rs`,
    `tests/renderer_cache_tests.rs`). Commit `7f9d480`.
  - T6.4: MCP stdio client `tantra-agent-mcp::client::{McpClient, McpClientError}` +
    `src/bin/mcp_stdio_server.rs` (process-spawned `tests/client_tests.rs`). Commit `7f9d480`.
  - T6.11 Ph 15: `_jq` universal response filter via `serde_json_path` (RFC 9535) —
    `tantra-agent-core::jq::apply_jq_filter` + server-side `apply_jq_to_envelope`
    (filters the full envelope, writes the first match into `"data"`). Commit `7f9d480`.
  - T6.5: `tantra-agent-report::notes::{ResearchNote, NoteVerdict}` (Typst/Markdown) +
    `tantra-agent-research::notes::research_note_from_verification`. Commit `7f9d480`.
  - T6.9: Track-4 engines in `tantra-agent-analytics` — `regime/hmm` (Baum-Welch + Viterbi,
    deterministic quantile init), `changepoint` (CUSUM + structural break), `copula`
    (Clayton/Gumbel, analytic tail dependence, seeded-bootstrap joint VaR), `validation`
    permutation tests (sign-flip Sharpe + shuffled win rate); `AnalyticsError` gained
    `InvalidArgument`. 5 MCP gateway tools (`RegimeToolService` +
    `AllocationToolService::multi_period_optimize` over ledger MPO). Commit `95ec800`.
    Fixed the stale `test_tool_portfolio_optimize_hrp_unimplemented` → `..._hrp`.
  - T6.6 (Track 1) verified shipped (`28629d0`). T6.7 Track-2 chat tools —
    `market_depth`/`institutional_flows` (MarketReader methods) + `economic_calendar`
    (new `CalendarReader` port), catalogue **19 → 22 read-only**; honest
    `PortError::unavailable` stubs citing market 0006 / data 0017 / data 0018. Commit `150bcae`.
- **Rename to Tantra (plan `tantra-labs` 0004, 2026-11-12)**: workspace renamed to `tantra-*`.
  **Pre-existing, non-rename failure**: `test_architectural_dependency_rules` (R4) fails —
  `tantra-agent-mcp` depends on `tantra-ledger-allocation`; R4 predates it (`ef879be`,
  2026-11-06). Decide: drop the dep or widen R4 to allow mcp (the MPO MCP tool keeps the dep).

- **Docs overhaul (2026-11-12)**: the historical `docs/roadmap/` was dismantled into
  `docs/design/`, `docs/gaps/`, `docs/plans/` and the whole set rewritten to match the source.
  - Design: `architecture.md` now 17 crates (chat-tools, forecast, panel real); new
    `code-chat-tools.md`; migration map all-complete; guardrails note the 2 remaining inline
    `#[cfg(test)]` blocks; `code-engines.md` corrected (analytics `cointegration.rs` +
    `tearsheet.rs` are orphaned files not declared in `lib.rs`).
  - Gaps: **0002 closed** (chat-tools crate, commit `f3f1cdc`); 0001 widened to two files
    (`ref_store.rs` + `research/verify/mod.rs`); 0003–0007 still open with corrected scopes
    (jobs scheduler exists — gap 0006 is now only the runtime-side bridge).
  - Plans: active plans renumbered to fix a numbering collision — **0008** (TDD cleanup, gap
    0001) and **0009** (persistent memory/MCP client/Phase E, gaps 0003–0007) replace the old
    `0002`/`0003` files; **0007** (MCP master guide) unchanged except status header (Phase 16
    SSE transport noted shipped).
- **Plan 0006 (MCP/chat tool catalog expansion) — complete**: Phases 1–4 implemented
  (2026-11-11/12) and recorded in `docs/plans/README.md` Implemented table. Full detail in the
  README + git history; catalogue is 19 read-only + 2 alert + 3 strategy tools.
- **Plans 0001–0005**: complete and merged — see `docs/plans/README.md` Implemented table.

## Immediate Next Step

- **Plan 0007 remaining tracks (open)**: Track 3 (T6.8 fundamentals / mutual funds / IPO —
  needs new `tantra-data` ingestion first: file data gaps before tool code per plan 0007 §3),
  Track 5 (T6.10 safe trade-intent bridge via NoFX approval queue + paper sandbox, strategy
  CRUD, tax — the largest remaining slice).
- **Plan 0010 (T6.12, Proposed)**: `institutional_flows` shipped with T6.7; add
  `delivery_analysis` + `oi_buildup` tools (file the delivery / OI-buildup Rust bridge gaps
  in `tantra-data`/`tantra-market` first) and the composite-signal strategy in
  `tantra-strategies`.
- **R4 architectural-dependency test** (pre-existing): decide whether to drop the
  `tantra-agent-mcp` → `tantra-ledger-allocation` dep or widen R4 to allow mcp.
- **Filing discipline**: new chat-tool categories need provider-repo gap docs (`tantra-data`
  next 0019+; `tantra-market` next 0007+) before tool code.

New **plan 0012** (approved design, 2026-11-13): agent-roster culmination — Prism Insight (13,
github.com/dragon1086/prism-insight) + Fincept (37, github.com/Fincept-Corporation/FinceptTerminal)
clubbed into a **14-persona desk** (9 existing + 5 new: trading-flow-analyst, industry-analyst,
information-analyst, investment-strategist, geopolitics-analyst) with 37 folded lenses (6 economic
schools → macro-strategist, 11 investor mentors → investment-strategist, 20 geopolitics modes →
geopolitics-analyst). **Docs/design/plan only — no code landed.** Design:
`docs/design/agent-roster-culmination.md`; plan: `docs/plans/0012-*`. UI wiring + winning-trade
flow tracked by `tantra-ui/docs/plans/0007`.

## Decisions & Discoveries

- Plan numbering collision fixed: active plans 0008/0009 replace the reused `0002`/`0003`
  numbers (implemented plans 0001–0006 keep theirs). Gap 0002 closed in the index; gap
  numbers are never reused per `.agents/rules/gap-tracking.md`.
- Chat catalogue: **19 read-only tools** (`test_total_read_only_catalogue_count` pins the count)
  **plus** the 2-tool alert bundle and 3 strategy tools. Content (Phase 3) and
  options-chain/corporate-actions (Phase 4) tools all went into the main all-ReadOnly bundle
  (unlike the LocalWrite alert CRUD, which sits outside so its count-pin invariant stays
  untouched). Plan 0006 §2's flat-registry decision stands (revisit at ~40).
- Phase 4 port semantics: `OptionContract.option_type` is the canonical FNO symbol-grammar
  string ("CE"/"PE", mirroring `tantra-market-finance`'s `OptionType`); `OptionsChain` carries
  the underlying + snapshot time, contracts ordered by expiry/strike/type; chain cap 500
  (`MAX_CHAIN_CONTRACTS`, mirroring `MAX_BARS_LIMIT`). `CorporateAction.action_type` is the
  canonical `"dividend"|"split"|"bonus"|"rights"` set mirroring
  `tantra_data.domain.corporate_actions`' value objects; ratios are exact `Quantity` pairs
  (`ratio_from`/`ratio_to`), dividends carry `Option<Money>` per-share amounts; list cap 20
  default / 50 (`DEFAULT_ACTIONS_LIMIT`/`MAX_ACTIONS_LIMIT`, content-tools precedent).
- Content port semantics: news `instruments` are free-form mention tags (never parsed as
  canonical `InstrumentId` — news text doesn't carry validated scrip refs); filings instruments
  are canonical. `filing_type` is the canonical `FilingType` string set from Python
  (`corporate_action`, `shareholding`, `insider_trade`, `fund_holding`, `annual_report`,
  `quarterly_result`, `other`). Both tools accept `limit` (default 20, cap 50) enforced at the
  tool layer via `effective_limit`.
- Filings upstream reality: `tantra-data` (Python) ingests filings (`LakehouseFilingsReader`,
  closed gap 0003) but no Rust consumer can reach that lakehouse — the standing
  `tantra-data` → Rust FFI bridge is now gap `tantra-data/docs/gaps/0007-filings-rust-bridge.md`
  (the market-reference sample stores in `tantra-runtime` stub against the same missing bridge).
  News has no ingestion anywhere → `tantra-data/docs/gaps/0006-news-ingestion.md`.
- `ToolRisk::LocalWrite` on the chat agent triggers the existing Interactive permission prompt
  per mutating call (`ToolTraits::new` also forces serial execution for non-ReadOnly) — no
  engine change was needed.
- `MarketReader` has exactly two implementors: `FeedMarketReader` (runtime-api-rest) and
  `RuntimeMarketReader` (runtime-api, seedable test adapter); the new `ContentReader` port has
  the one `FeedContentReader` implementor (alert-port precedent: no seedable test adapter until
  a consumer needs one).
- Toolchain's clippy (rust-1.97) flags `map_or(true, ..)` as `unnecessary_map_or`; use
  `is_none_or` in new code.
- Live GGUF inference tests (`tantra-agent-llm/tests/candle_gguf_tests.rs::test_live_zenfinance_*`)
  are `#[ignore]`d: whenever `models/zenfinance-3b-Instruct.Q4_K_M.gguf` exists (2 GB) they run
  minutes of real 3B inference inside the default gate — machine-dependent and CI-impossible.
  Same policy as 4B's OmniRoute live tests; runnable via `cargo test -p tantra-agent-llm
  --test candle_gguf_tests -- --ignored`.

## Operating Invariants

- Canonical profile `prod`: `cargo build/check/test --profile prod`; `make prod` end-of-turn (§11).
- Tests in `tests/` parallel to `src/`; no in-file `#[test]`.
- Zero floats in financial paths; `rust_decimal::Decimal`.
- `cargo fmt --check`, `cargo clippy --workspace --all-targets --profile prod -- -D warnings`,
  `cargo test --workspace --profile prod` all green (verified 2026-11-12).
- Gap/plan numbering per-repo, sequential, never reused (`.agents/rules/gap-tracking.md`).
