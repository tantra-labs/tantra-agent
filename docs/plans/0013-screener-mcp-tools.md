# 0013 — Screener, Universe, Alert & Watchlist MCP Tools

**Status**: Proposed

## 0. Executive Summary & Objective

Give agents a full-market screening surface over MCP: browse the screen catalog, discover metrics,
run a screen across a whole universe, resolve universes, look up classification, read QVM scores,
manage alert rules and watchlists. Tools are defined against ports in `tantra-agent-*` and return
`PortError::unavailable` until [tantra-runtime 0005](../../../tantra-runtime/docs/plans/0005-group-screener-alerts.md)
wires implementations.

References: research [`trendlyne-screener-alerts-mcp.md`](../../../docs/research/trendlyne-screener-alerts-mcp.md) (F8);
root roadmap [`0007`](../../../docs/plans/0007-screener-taxonomy-and-alerts.md).
Depends on: [tantra-core 0004](../../../tantra-core/docs/plans/0004-classification-and-alert-wire-types.md),
[tantra-data 0008](../../../tantra-data/docs/plans/0008-screen-dsl-catalog-qvm.md).

## 1. Context & Gaps Covered

`tantra-agent-mcp/src/tools.rs` `list_tool_definitions()` is hand-written `json!`; `server.rs`
`call_tool(name, args)` matches by name; results ride `TantraResultEnvelope`. Chat tools
(`tantra-agent-chat-tools/src/catalog.rs`) include `ScreenerQueryTool`, FilingsSearch, NewsSearch,
Alerts (`PriceAlert`, `AlertReader`/`AlertWriter`). Contrast: the Trendlyne MCP offers at most 10
stocks per call and no screen tool; Tantra's `screen_run` scans the whole universe.

## 2. Component Specifications & Phases

### Phase A — Ports (`tantra-agent-tools` stays pure, sync-free of tokio per R3)
`ScreenerPort`, `UniversePort`, `AlertRulePort`, `WatchlistPort` with typed DTOs from tantra-core and
typed errors; explicit `PortError::unavailable(...)` until wired.

### Phase B — MCP tools (input JSON schemas defined with each)
- `screen_catalog{category?, query?}` browse presets.
- `metric_search{query, limit}` parameter discovery (the pattern Trendlyne gets right).
- `screen_run{preset_id | group, universe, sort, limit}`; large results returned as `$tantra/ref` handles
  via `DataReferenceStore`, with a bounded inline preview.
- `universe_resolve{name, as_of?}`, `classification_lookup{instruments[]}`, `qvm_score{instruments[], as_of?}`.
- `alert_rule_create|list|delete`, `watchlist_create|list|add|remove` (LocalWrite tier, Caller-scoped).
- Register in `list_tool_definitions` and `call_tool`; discoverable via `tantra_search_tools`.

### Phase C — Chat tools
- Extend `ScreenerQueryTool` to group rules/presets; extend the Alerts tool from price to group rules.
- Follow the plan 0006 category-addition recipe (DTO+port -> tool -> catalog -> runtime wiring -> count-test bump).

## 3. Test Plan (TDD)

In `tests/`: schema snapshot per tool, unavailable-port behaviour, `$tantra/ref` large-result
handling, write tools require LocalWrite, catalogue count-pin bump, `dependency_rules.rs` still green.

## 4. Dependencies & Tier Notes

R1: no `tantra-runtime-*`/`exec`/`market` deps. R3: `tantra-agent-tools` pure. R4: any wiring needing
market/allocation data belongs to tantra-runtime composition, not `tantra-agent-mcp`; only
`tantra-agent-chat-tools` may use `tantra-ledger-*`.

## 5. Open Questions

- Inline preview row cap for `screen_run` before switching to a ref handle.
- Watchlist ownership vs portfolio objects: separate port or reuse portfolio store?
