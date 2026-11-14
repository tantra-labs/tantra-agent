# HANDOFF — tantra-agent

**Date**: 2026-11-14 · Repo: `tantra-agent` · Branch: `main`

## Phase / Slice Status

- **T6.10 (plan 0007 Track 5 — tool surface, 2026-11-14)**: safe trade intent / strategy
  CRUD / tax shipped:
  - Trade intent (Ph 12, NoFX approval-queue pattern): `TradeIntentSink` port + `propose_trade`
    LocalWrite tool formulating a closed `Decision::PlaceOrder`; runtime sink answers
    unavailable citing runtime gap 0007 item 17.
  - Strategy CRUD (Ph 13, Jesse-MCP): `StrategyStore` port + `strategy_manage` LocalWrite
    tool (create/list/inspect/delete); runtime store answers unavailable citing item 18.
  - Tax & statements (Ph 14): `TaxReportReader` port + pure `compute_capital_gains`
    (STCG 20% flat, LTCG 12.5% over ₹1.25L exemption, exact Decimal) + `tax_report`/
    `ledger_statement` ReadOnly tools with markdown statements; runtime reader answers
    unavailable citing item 19.
  - Catalogue 30→32 read-only + 2 LocalWrite trade tools; 25 unit tests green; fmt + clippy
    `-D warnings` clean. Deep wiring (FileJobRegistry enqueue, strategy store, matched-lot
    bridge) tracked by runtime gap 0007 items 17–19.
- **T6.8 (plan 0007 Track 3 — tool surface, 2026-11-14)**: fundamentals / mutual funds /
  IPO chat-tool surface shipped:
  - 3 new ports + 6 new ReadOnly tools (catalogue **24 → 30**): `FundamentalReader`
    (`fundamental_ratios`/`financial_statements`), `FundReader` (`fund_search`/
    `fund_holdings`), `IpoReader` (`ipo_calendar`/`ipo_subscription`) — exact `Decimal`,
    honest `PortError::unavailable` stubs citing new `tantra-data` gaps 0020 (fundamentals),
    0021 (mutual funds), 0022 (IPO).
  - Runtime: `FeedFundamentalReader`/`FeedFundReader`/`FeedIpoReader` adapters in
    `tantra-runtime-api-rest` (`chat_fundamental.rs`/`chat_fund.rs`/`chat_ipo.rs`), wired in
    `ChatRuntime::from_env`.
  - `register_read_only_tools` gained the 3 reader params (`#[allow(clippy::too_many_arguments)]`);
    runtime gap 0007 doc appends items 14–16. Data ingestion tracked by the three gaps.
- **T6.12 (plan 0010 — Implemented, 2026-11-14)**: Institutional / smart-money detection
  suite Phases 1–5 landed:
  - `delivery_analysis` + `oi_buildup` chat tools (catalogue **22 → 24** read-only):
    `DeliveryBar`/`OiBuildupSnapshot` DTOs + `MarketReader::delivery_stats`/`oi_buildup`
    port methods, exact `Decimal` throughout; honest `PortError::unavailable` stubs citing
    the new `tantra-data` gap 0019 (delivery Rust bridge) and `tantra-runtime` gap 0007
    item 12 (no OI store wired).
  - Runtime wiring: `FeedMarketReader` stubs + seedable `RuntimeMarketReader`
    (`set_delivery_stats`/`set_oi_buildup`, filtered/capped reads) + tests.
  - Composite signal strategy: `tantra_strategies/strategies/composite_signal.py`
    (`CompositeSignalConfig`, pure `compute_composite_signal` 4-leg + regime gate,
    `CompositeSignalStrategy` with declarative `Parameter`s) + 14 tests in
    `tests/strategies/test_composite_signal.py`.
  - Plan 0010 doc retired to Implemented (git history); remaining real-data wiring tracked
    by `tantra-data` gaps 0017/0019 + `tantra-market` 0006 + runtime gap 0007 item 13.
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
  README + git history; catalogue is 32 read-only + 2 alert + 2 trade + 3 strategy tools.
- **Plans 0001–0005**: complete and merged — see `docs/plans/README.md` Implemented table.

## Immediate Next Step

- **T6 track complete (2026-11-14)**. Follow-ons tracked by upstream gaps/docs:
  - Track 3 data ingestion: `tantra-data` gaps 0020–0022 (fundamentals / mutual funds / IPO).
  - Track 5 deep wiring: runtime gap 0007 items 17 (FileJobRegistry-backed `TradeIntentSink`
    + paper sandbox), 18 (strategy-manifest `StrategyStore`), 19 (matched-lot `TaxReportReader`
    bridge).
  - Plan 0010 (T6.12, Implemented): close `tantra-data` 0017/0019, `tantra-market` 0006, and
    wire `ParquetOpenInterestStore` + `BuildupAnalyzer` into `FeedMarketReader::oi_buildup`
    (runtime gap 0007 item 13).
- **R4 architectural-dependency test** (pre-existing): decide whether to drop the
  `tantra-agent-mcp` → `tantra-ledger-allocation` dep or widen R4 to allow mcp.
- **Filing discipline**: new chat-tool categories need provider-repo gap docs (`tantra-data`
  next 0023+; `tantra-market` next 0007+) before tool code.

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
- Chat catalogue: **32 read-only tools** (`test_total_read_only_catalogue_count` pins the count)
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
