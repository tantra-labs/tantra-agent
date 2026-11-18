# 0016 — `Experimental-Analysis` AI Assistant & Historical Outlier News Tools (`tantra-agent`)

**Status**: Proposed (2026-11-18) — Track **T20.4**

Master plan: root [`docs/plans/0014-experimental-analysis-statistical-workbench.md`](../../../docs/plans/0014-experimental-analysis-statistical-workbench.md).  
Depends on: [`tantra-runtime` plan 0009](../../../tantra-runtime/docs/plans/0009-experimental-analysis-statistical-rest-api.md).

---

## 0. Executive Summary & Objective

Add `ReadOnly` chat and MCP tools enabling the `tantra-agent` copilot (`ChatRail` in `tantra-ui-web`, `tantra tui`, `tantra ask`) to query multi-asset statistical studies and retrieve dated historical macro/market news headlines for extreme outlier windows (`Historical Global News & Events` panel in `Experimental-Analysis`).

---

## 1. New Read-Only Tools (`tantra-agent-chat-tools` + `tantra-agent-mcp`)

1. **`experimental_study_summary`**:
   - Calls `/api/v2/experimental-analysis/returns` (or internal `StudyReader` port) for a given `instrument_id`, `asset_class` (`Index`/`F&O`/`ETF`/`Equity`/`Forex`/`Commodity`), `period`, and `calc_type`.
   - Returns exact KPIs, current Markov streak prediction, `Probability-Based Levels` (e.g., 80%/90%/95% bounds for strike sizing), and recent forward-conditional behavior wrapped in `TantraResultEnvelope` (`Rule R1–R4` compliant).
2. **`historical_outlier_news`**:
   - Accepts `{ instrument_id, asset_class, window_start, window_end, keywords }`.
   - Queries internal news/disclosures archive first (`ContentReader::news_search` / Lakehouse news store) and augments via `tantra_web_search` / RSS date-bounded queries with default domain keyword packs (`market economy finance global crash` for Indices/F&O, `crude oil opec gold fed` for Commodities, `central bank rbi fed currency` for FX).
   - Caches resolved `(asset_class, window_start, window_end, keywords)` bundles to avoid redundant external lookups and surfaces dated source links cleanly in both the UI panel and chat answers.

---

## 2. Prompt & Grounding Alignment

- Update persona guidance only where needed so assistant answers cite exact sample counts, period schedules (e.g., NSE Sep 3, 2025 expiry shift), and never fabricate return percentiles or historical headlines when external search returns empty.
- Update tool catalogue count pins and unit/integration tests in `tantra-agent-mcp/tests/`.
