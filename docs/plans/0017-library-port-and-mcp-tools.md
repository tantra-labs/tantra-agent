# 0017 — Indicator Library Port & MCP Tools

**Status**: Proposed (2026-11-20) — design only. Parent: root plan 0016 (Track T22, phase T22.3–T22.4).

## 0. Executive Summary & Objective

Give agents one door to the Concept & Indicator Library (root plan 0016): an `IndicatorLibraryPort` trait in
`tantra-agent-core` (implemented in `tantra-runtime`, never in `tantra-agent-*`), four MCP tools
(`tantra_library_search`, `tantra_library_describe`, `tantra_indicator_compute`, `tantra_signal_study`), and
retirement of the hand-written 11-entry `IndicatorCatalog` / `get_indicator_catalog`.

## 1. Context & Gaps Covered

- `tantra-agent-core/src/indicators.rs` hand-writes `IndicatorCategory`, `ParameterSpec`, `IndicatorSpec`,
  `IndicatorCatalog { indicators: HashMap<String, IndicatorSpec> }` (`new()`, `list()`), 11 entries. It duplicates
  market truth, drifts, and cannot compute.
- MCP tools are static JSON in `tantra-agent-mcp/src/tools.rs::list_tool_definitions() -> Vec<serde_json::Value>`;
  dispatch is a `match name {...}` in `src/server.rs` (~line 262, `tools/call`).
- Rules R1–R4 (`tantra-agent-chat/tests/dependency_rules.rs`): `tantra-agent-*` must not depend on
  `tantra-runtime-*`/`tantra-exec-*`/`tantra-market-*`; `tantra-agent-tools` must not use tokio/async-trait/reqwest/sqlx;
  market-needing MCP wiring belongs in the `tantra-runtime` composition root.

## 2. Component Specifications & Phases

### 2.1 Port (T22.3) — `tantra-agent-core/src/library_port.rs`

DTOs come from `tantra_core::library` (root plan 0016 §3 decision 1): `EntryDescriptor`, `ParamSpec`,
`OutputSpec`, `EntryKind`, `Determinism`. Agent-side request/response types (also schemars) live next to the port.

```rust
#[async_trait::async_trait]   // tantra-agent-core may use async-trait; confirm vs R3 scope (only agent-tools is restricted) in T22.3
pub trait IndicatorLibraryPort: Send + Sync {
    async fn list(&self, q: ListQuery) -> Result<Vec<EntryDescriptor>, LibraryPortError>;      // kind, family
    async fn search(&self, q: &str, limit: usize) -> Result<Vec<SearchHit>, LibraryPortError>;
    async fn describe(&self, id: &str) -> Result<EntryDoc, LibraryPortError>;                   // descriptor + concept Markdown + dependents
    async fn compute(&self, req: ComputeRequest) -> Result<ComputeResult, LibraryPortError>;
    async fn signal_study(&self, req: SignalStudyRequest) -> Result<SignalStudyResult, LibraryPortError>;
}
#[derive(thiserror::Error, Debug)]
pub enum LibraryPortError {
    NotFound(String), InvalidParams { id: String, detail: String }, InputQuality { id: String, detail: String },
    LimitExceeded { what: String, max: u64 }, Unavailable(String), Internal(String),
}
```
`Unavailable` is returned (not empty results) when no adapter is wired (`PortError::unavailable` convention).
`ComputeRequest { id, params: BTreeMap<String, ParamValue>, instrument, timeframe, from, to, columns?, data_commit? }`;
`ComputeResult::Inline(Frame) | Handle { result_id, rows, columns }`. `SignalStudyRequest/Result` mirror
runtime plan 0010 §2.4 (shared via `tantra-runtime-api` DTOs where possible; to confirm in T22.4).

### 2.2 MCP tools (T22.3–T22.4) — `tantra-agent-mcp/src/tools.rs` + `server.rs`

New service struct `LibraryToolService { port: Arc<dyn IndicatorLibraryPort> }`; one `match` arm per tool.
All schemas set `"additionalProperties": false`.

| Tool | Input schema (sketch) | Returns |
|---|---|---|
| `tantra_library_search` | `{q: string(1..200), kind?: enum[family,concept,primitive,indicator,composite,detector,recipe], family?: string, limit?: int(1..50, default 10)}` | `[{id, kind, family, summary, aliases}]` |
| `tantra_library_describe` | `{id: string, format?: enum[json,md] = md}` | `md`: concept page Markdown "copy for LLM" (descriptor table + body + implementations + related); `json`: `EntryDescriptor` |
| `tantra_indicator_compute` | `{id, params?: object, instrument: string, timeframe: string, from: date, to: date, columns?: [string], tail?: int(1..500)}` | inline frame (small) or `{result_id, rows, columns}` handle readable via existing `tantra_read_result` |
| `tantra_signal_study` | `{detector: {id, params?}, filters?: [{id, params?, expect}], universe: [string], timeframe, from, to, horizons?: [int] (default [1,3,5,10,20]), cost_model?: bool}` | summary stats per horizon + `{events_result_id}` handle |

Descriptions are discoverable through the existing `tantra_search_tools` / `tantra_describe_tool` meta-tools.
Inline cap: frames above ~200 rows or ~16 KiB return a handle (`tantra_read_result` paging).

### 2.3 Validation (root plan 0016 §4.4)

Agent input is untrusted: serde structs use `deny_unknown_fields`; the port adapter validates `{id, params}`
against the manifest (unknown param, bounds, enum, max bars/window, max DAG depth, instruments x bars x entries
cap) and returns `InvalidParams`/`LimitExceeded`; MCP layer maps errors to JSON-RPC tool errors with typed
`code` (no raw strings of internals). `tantra_signal_study` and `compute` are read-only research; no order
path. Agent-varied recipes are research artefacts only.

### 2.4 Retiring `IndicatorCatalog`

1. T22.3: `get_indicator_catalog` stays one release as an **alias** serving `port.list(kind=indicator)` mapped to the old JSON shape
   (marked `deprecated` in the tool description); `IndicatorCatalog` becomes a thin view over the port.
2. Next release: delete `IndicatorCatalog`, `IndicatorSpec`, `ParameterSpec`, `IndicatorCategory`, `IndicatorCatalogService`
   and their tests; update `tantra_search_tools` index and docs. Record closure in README.

### 2.5 Chat copilot use (agent plan 0016)

The Experimental-Analysis copilot (plan 0016, `tantra-agent-chat-tools`) gets the same tools via the port:
search -> describe (reads concept Markdown to explain) -> compute (overlay data for the UI) -> signal_study
(feeds the summary the copilot narrates). Copilot may not bypass validation; the "ask copilot" hook in
ui-web plan 0017 sends `{entry_id, params, study_context}` as chat context.

### 2.6 R1–R4 compliance

Port and DTOs depend only on `tantra-core` (R1). `LibraryToolService` is in `tantra-agent-mcp` and sees only
`Arc<dyn IndicatorLibraryPort>`. `tantra-agent-tools` gains no new deps (R3). The adapter, which needs
`tantra-market-library`, lives in `tantra-runtime` (R4); wiring in `tantra-runtime`'s bootstrap composition root.
`chat-tools` and `trader-loop` stay decoupled (R2).

## 3. Tests & Verification

- Tests in `crates/*/tests/` only. Add: `tantra-agent-core/tests/library_port_dto.rs` (serde/schema round-trip, `deny_unknown_fields`),
  `tantra-agent-mcp/tests/library_tools.rs` with an in-memory fake port: tool listing, schema strictness, each tool happy path,
  invalid params, `Unavailable` when port unwired, handle path via `tantra_read_result`, alias `get_indicator_catalog` parity.
- `dependency_rules.rs` R1–R4 stays green (never weakened).
- Gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `make check`, `make test`, `make prod`.

## 4. Open Questions

1. Is `async-trait` acceptable in `tantra-agent-core` (R3 restricts only `tantra-agent-tools`)? Confirm in T22.3; else return boxed futures.
2. Where do `SignalStudyRequest/Result` live to avoid duplicating runtime DTOs — `tantra-core::library` or agent-core? (T22.4)
3. Deprecation window length for `get_indicator_catalog` (one release proposed).
