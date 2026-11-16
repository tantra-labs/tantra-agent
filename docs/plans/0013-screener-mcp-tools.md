# 0013 — Screener, Universe, Alpha-Alerts & Watchlist MCP Tools

**Status**: Proposed (updated 2026-11-16 after architectural review & Alpha-Alerts expansion)

## 0. Executive Summary & Objective

Give agents a full-market screening and Alpha-Alerts monitoring surface over MCP and chat: browse
the screen catalog by category, discover metrics, run a screen across a whole universe, resolve
point-in-time universes, look up classification, read QVM scores, manage multi-universe
group/event alert rules, subscribe to one-click Alpha-Alert packs, inspect digests, and manage
watchlists.

Async I/O ports live in **`tantra-agent-chat-tools` / `tantra-agent-mcp`** (keeping
`tantra-agent-tools` strictly pure and sync per Rule R3) and return `PortError::unavailable` until
[tantra-runtime 0007](../../../tantra-runtime/docs/plans/0007-group-screener-alerts.md) wires
implementations (Rule R4).

References: research [`trendlyne-screener-alerts-mcp.md`](../../../docs/research/trendlyne-screener-alerts-mcp.md) (F8);
root roadmap [`0010`](../../../docs/plans/0010-screener-taxonomy-and-alerts.md).
Depends on: [tantra-core 0004](../../../tantra-core/docs/plans/0004-classification-and-alert-wire-types.md) (converged Screener DSL & slim classification VOs),
[tantra-data 0008](../../../tantra-data/docs/plans/0008-screen-dsl-catalog-qvm.md) (screen catalog & QVM),
[tantra-runtime 0007](../../../tantra-runtime/docs/plans/0007-group-screener-alerts.md) (Alpha-Alerts DTOs & port wiring).

## 1. Context & Gaps Covered

`tantra-agent-mcp/src/tools.rs` `list_tool_definitions()` is hand-written `json!`; `server.rs`
`call_tool(name, args)` matches by name; results ride `TantraResultEnvelope`. Chat tools
(`tantra-agent-chat-tools/src/catalog.rs`) include lifted pure `ScreenerQueryTool` (which stays
pure/sync in `tantra-agent-tools` for deterministic replay), `FilingsSearch`, `NewsSearch`, and
single-stock `PriceAlert` (`AlertReader`/`AlertWriter` in `tantra-agent-chat-tools/src/ports/alert.rs`).
Contrast: Trendlyne's MCP offers at most 10 stocks per call and no full-market screen tool;
Tantra's `screen_run` scans the whole universe and pairs with native Alpha-style alerting.

## 2. Component Specifications & Phases

### Phase A — Async Ports (`tantra-agent-chat-tools` / `tantra-agent-mcp`, NOT `tantra-agent-tools`)
- Keep `tantra-agent-tools` strictly pure, deterministic, and sync (`ScreenerQueryTool` untouched per Rule R3).
- Define async `#[async_trait]` ports in `tantra-agent-chat-tools::ports` (reused by `tantra-agent-mcp` without violating R1/R2):
  - `ScreenerPort` (catalog browse, metric search, full-universe scan)
  - `UniversePort` (point-in-time universe resolve, classification lookup, QVM lookup)
  - `AlertRulePort` (CRUD, snooze, events, digests, preset notification packs — superseding/extending `AlertReader`/`AlertWriter`, preserving optional `workspace_id` filtering & attribution)
  - `WatchlistPort` (create, list, add/remove instruments)
- Typed DTOs from `tantra-core` 0004 (screener/classification) and `tantra-runtime-api` / shared L3 alert contract; explicit `PortError::unavailable(...)` until wired by `tantra-runtime`.

### Phase B — MCP tools (`tantra-agent-mcp`, input JSON schemas defined with each)
- **Screening & Discovery**:
  - `screen_catalog{category?, query?}` — browse presets by `ScreenCategory` slug (`tantra-data` 0008).
  - `metric_search{query, limit}` — parameter discovery before querying.
  - `screen_run{preset_id | group, universe, sort, limit}` — full-universe scan; large results returned as `$tantra/ref` handles via `DataReferenceStore` with a bounded inline preview.
  - `universe_resolve{name, as_of?}`, `classification_lookup{instruments[]}`, `qvm_score{instruments[], as_of?}`.
- **Alpha-Alerts & Digests (`LocalWrite` tier, `Caller`-scoped)**:
  - `alert_preset_list{category?}` & `alert_preset_subscribe{preset_id, universes[], delivery_mode, channels[], workspace_id?}` — one-click Alpha-Alert packs over portfolios/watchlists.
  - `alert_rule_create{name, universes[], condition, frequency, delivery, cooldown_secs?, workspace_id?}` — supports both `ScreenEdge` (preset or inline `ScreenerFilterGroup`) and `EventMatch` (results, corp actions, filings, deals, QVM transitions).
  - `alert_rule_list{universe?, workspace_id?, enabled?}`, `alert_rule_update{...}`, `alert_rule_snooze{rule_id, until}`, `alert_rule_delete{rule_id}`.
  - `alert_events_list{rule_id?, workspace_id?, since?, limit?}` & `alert_digest_prefs{mode, channels, quiet_hours?}`.
- **Watchlists (`LocalWrite` tier, `Caller`-scoped)**:
  - `watchlist_create|list|add|remove`.
- Register all in `list_tool_definitions` and `call_tool`; discoverable via `tantra_search_tools`.

### Phase C — Chat tools (`tantra-agent-chat-tools`)
- Extend chat `ScreenerQueryTool` adapter / add `CatalogScreenTool` for group rules & catalog presets; evolve `PriceAlertTool` into `AlphaAlertTool` supporting multi-universe group/event alerts and preset subscriptions while keeping single-price convenience syntax (`workspace_id` optional).
- Follow the plan 0006 category-addition recipe (DTO+port -> tool -> catalog -> runtime wiring -> count-test bump).

## 3. Test Plan (TDD)

In `tests/`:
- Schema snapshot per tool, `PortError::unavailable` behaviour, `$tantra/ref` large-result handling.
- Write tools require `ToolRisk::LocalWrite` + `Caller` scoping.
- Catalogue count-pin bump; `dependency_rules.rs` green (verifying R1–R4, including R3 purity of `tantra-agent-tools`).

## 4. Dependencies & Tier Notes

R1: no `tantra-runtime-*`/`exec`/`market` deps (alert/catalog DTOs needed by both `tantra-agent` and `tantra-runtime` live in `tantra-agent-chat-tools` or a shared pure-types module, since `tantra-runtime` depends on `tantra-agent-*`). R3: `tantra-agent-tools` stays sync/tokio-free. R4: all wiring lives in `tantra-runtime` composition; only `tantra-agent-chat-tools` may use `tantra-ledger-*`.

## 5. Open Questions

- Inline preview row cap for `screen_run` before switching to a `$tantra/ref` handle.
- Watchlist ownership vs portfolio objects: separate `WatchlistPort` store or unified workspace list?
