# 0011 — Elephant Edge Level-Map & OI-Wall Chat Tools

**Status**: Proposed
**Target Scope**: `tantra-agent-chat-tools`, `tantra-runtime-api(-rest)`
**Upstream Dependencies**: `tantra-core` (plan 0003 / gap 0002 — kernel level-map
& options-positioning wire types), `tantra-market` (gap 0005 — indicator feeds;
gaps 0001/0002 already closed for chain + OI), `tantra-data` (gap 0016 —
snapshot lakehouse)
**Overlaps with / cites**: [plan 0010](0010-institutional-smart-money-detection-suite.md)
(OI-buildup chat tools) — this plan adds the *level-map* read surface on top of
the same data legs; no duplication of the buildup classification itself.

## 0. Objective

Expose the Elephant Edge (EE+) day's level map as **read-only chat tools** so
the AI research loop and `tantra-cli ask` can reason about support/resistance
confluence exactly as the strategy does — under the untrusted-AI principle
(closed `Decision` grammar downstream, `ToolRisk::ReadOnly`).

## 1. Tools

1. **`level_map`** — `MarketReader::level_map(&self, instrument, date?) ->
   Result<LevelMap, PortError>`: the session's `LevelMap` (Elephant zones, OI
   walls, gamma flip, max pain, PDH/PDL, Order Blocks) with confluence scores,
   serialized from `tantra-core` plan 0003 types.
2. **`oi_walls`** — `MarketReader::oi_walls(&self, instrument, expiry?) ->
   Result<OiWallsSnapshot, PortError>`: top-2 CE/PE OI strikes + ΔOI building
   scores + PCR (the Layer A/B/C inputs), reusing the closed
   `ChainReader`/`OpenInterestStore` plumbing.

Both registered in the main all-ReadOnly bundle (catalogue count +2 with the
assertion pin update in `tests/unit_tests.rs`, per plan 0006 §4's recipe).

## 2. Runtime Wiring (`tantra-runtime`)

- `FeedMarketReader`: honest `PortError::unavailable` citing the upstream gaps
  until they close (same pattern as `options_chain`/`corporate_actions`).
- `RuntimeMarketReader`: gains `set_level_map` / `set_oi_walls` seedable
  stores (empty ⇒ `unavailable`), tested in `chat_adapters_tests.rs`.

## 3. Subagent Invariants

1. Zero inline tests; all tests in `tests/`.
2. Zero floats — every level, OI value, and ratio is `Decimal`.
3. Honest degradation — never fabricate levels or walls.
4. Catalogue-count pin updated in the same change as tool registration.
5. Gates: `cargo fmt --check`, `clippy -- -D warnings`, `cargo test --workspace
   --profile prod`, `make prod` end-of-turn.

## 4. Related

- Master plan: root [`docs/plans/0005-elephant-edge-strategy-platform.md`](../../../docs/plans/0005-elephant-edge-strategy-platform.md)
- Strategy spec: [`tantra-strategies/docs/plans/0001-elephant-edge-strategy.md`](../../../tantra-strategies/docs/plans/0001-elephant-edge-strategy.md)
- Existing legs: plan 0010 (institutional flow detection suite)
