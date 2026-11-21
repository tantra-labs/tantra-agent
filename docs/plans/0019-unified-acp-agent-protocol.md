# 0019 — One agent protocol for TUI, web and CLI/Python SDK (ACP + Tantra extensions)

**Status**: In progress. Phases 0–2 done; Phase 3 done for the TUI (`tantra ask` remaining); Phases 4–6 open.

## 0. Executive Summary & Objective

Today three clients reach the Tantra agent three different ways:

| Client | Path today | Wire |
|---|---|---|
| Web (`tantra-ui-web`) | Next.js `/api/chat` BFF → runtime `GET /ws/chat` | bespoke: `{"text"}` in, `SessionEvent` JSON out |
| TUI (`tantra-ui/crates/tantra-tui`) | in-process `tantra_agent_chat::run_turn` | none (Rust types) |
| CLI / Python SDK | in-process / none | none |

Objective: every client speaks **one language**, the **Agent Client Protocol (ACP)**:
JSON-RPC 2.0 with `initialize`, `session/new`, `session/prompt`, `session/cancel`,
`session/update` notifications and `session/request_permission`, carrying Tantra
specifics in ACP's `_meta` extension point. Each entry point chooses its LLM.

Decisions (user, 2026-11-21): ACP + Tantra extensions; TUI/CLI run the agent **embedded or
remote** behind the same client.

## 1. Context & Gaps Covered

- `tantra-agent-chat` already has the session/turn engine (`run_turn`, `SessionEvent`,
  permission prompts, `ToolFinished.blocks: Vec<DisplayBlock>` since `db826c1`).
- `tantra-runtime-api-rest` already has a provider registry with per-turn model override
  (`chat.rs`), auth/`Caller` scoping and SQLx session persistence for `/ws/chat`.
- Local GGUF models gained per-family prompt + tool-call formats (`709476e`).
- Missing: an ACP agent adapter, an ACP client usable from Rust, TS and Python, an ACP
  endpoint on the runtime, and per-client model selection on the wire.

## 2. Component Specifications & Phases

### Protocol mapping (normative)

| Tantra | ACP |
|---|---|
| connect | `initialize` (protocolVersion, capabilities; `_meta.tantra.models`: models the host offers) |
| new / resume session | `session/new` / `session/load`; `_meta.tantra.model` picks the LLM; `_meta.tantra.workspace` scopes it |
| user turn | `session/prompt` (text content blocks) → `PromptResponse.stopReason` |
| `TextDelta` / `ThinkingDelta` | `session/update` `agent_message_chunk` / `agent_thought_chunk` |
| `ToolStarted` | `session/update` `tool_call` (status `in_progress`, title = summary, kind from tool risk) |
| `ToolFinished` | `session/update` `tool_call_update` (status `completed`/`failed`, text content = summary); **`_meta.tantra.blocks` = `Vec<DisplayBlock>`** |
| `PermissionRequested` | agent→client `session/request_permission` (allow_once / allow_always / reject_once) |
| cancel | `session/cancel` notification → `CancellationToken` |
| change LLM mid-session | ext method `_tantra/set_model` (ACP `session/set_model` once stable) |
| `TurnEnded` | `PromptResponse.stopReason` (`end_turn`, `max_tokens`, `refusal`, `cancelled`, max-turn) |

LLM configuration rule: the **host** owns provider construction and secrets. Clients only
select among `_meta.tantra.models` by id; API keys never cross the wire. Every LLM is an
OpenAI-compatible endpoint listed in a `tantra.jsonc` config file (see
`docs/design/provider-config.md`); there are no LLM env vars and no in-process GGUF provider.
In embedded mode the client process *is* the host, so `--config` and `--model` configure it locally.

### Phase 0 — provider config file (done)
- `tantra-agent-llm::providers`: opencode-style `provider` block, `{env:}`/`{file:}` references,
  user + project merge, `ConfigCatalog` -> `StaticCatalog`; `ModelCatalog` moved to `tantra-agent-llm::catalog`.
- Candle GGUF provider, chat-template format layer and `TANTRA_LLM_*` env API removed.

### Phase 1 — `tantra-agent-acp` crate (agent tier, this repo)
- `ModelCatalog` port (trait): `list() -> Vec<ModelInfo>`, `provider(id) -> Result<Arc<dyn LlmProvider>, CatalogError>`; typed errors.
- `AcpAgent`: implements the ACP agent side over `run_turn` + `ToolSet` + `SessionStore`
  (injected), per-connection session map (bounded), mapping table above.
- Transports: generic over `AsyncRead + AsyncWrite` (newline-delimited JSON-RPC, as ACP
  stdio); `tantra-acp` stdio binary; in-process duplex helper for embedded clients.
- `AcpClient` (Rust): connect embedded (duplex), stdio child, or WebSocket URL; yields a
  typed event stream the TUI/CLI fold (reuses the TUI's `AgentStreamUpdate` semantics).
- Dependency rules R1–R4 hold: no `tantra-runtime-*`/`tantra-exec-*`/`tantra-market-*` deps.
- Tests: golden JSON-RPC transcripts (initialize → session/new → prompt with a scripted
  provider + fake tool → updates incl. `_meta.tantra.blocks` → stopReason), permission
  round-trip, cancel, unknown model → typed error.

### Phase 2 — runtime ACP endpoint (`tantra-runtime`)
- `GET /api/v1/acp` WebSocket hosting `AcpAgent` per connection; auth middleware + `Caller`
  scoping (sessions keyed by `(caller_id, workspace)`); `ModelCatalog` backed by the existing
  chat provider registry; SQLx `SessionStore`; bounded frame size and per-socket session cap.
- `/ws/chat` kept as a deprecated shim until Phase 4 lands, then removed.
- **Done 2026-11-21** (runtime `756d267`): `acp_ws_handler` on `/api/v1/acp` (Authenticated scope,
  `Caller` extracted), full chat tool set + `present_view` (agent plan 0020), host system prompt
  appends `PRESENT_VIEW_GUIDANCE`, 64-connection semaphore, 1 MiB frames, 256 KiB duplex bridge.
  **Deviations / follow-ups**:
  - The `SessionStore` isn't namespaced by `caller_id`. Isolation instead comes from per-connection
    sessions with `loadSession: false`; caller-keyed resume is a follow-up.
  - On disconnect the bridge aborts its tasks without draining output that's still buffered.

### Phase 3 — TUI + CLI (`tantra-ui`)
- **Done 2026-11-21 (TUI)**:
  - The TUI runs agent turns over `AcpClient` (ui `86d35ee`). It's embedded by default via
    `connect_embedded`; `--server ws://…/api/v1/acp [--token-file PATH]` connects remotely via the new
    `connect_ws` (agent `c91301a`; rustls, 1 MiB frames, bearer header). No `TANTRA_SERVER` env var.
  - `/model` lists or switches models. `/auto` toggles a shared `ClientOptions.approve` flag
    without reconnecting. `/new` and Ctrl+N reconnect to start a fresh session.
  - **Remaining**: `tantra ask` still calls `run_turn` directly.
- TUI swaps its direct `run_turn` for `AcpClient`: embedded by default, `--server URL` /
  `TANTRA_SERVER` for remote; `--model` flag + `/model` palette command (lists
  `_meta.tantra.models`). Analysis page reads `_meta.tantra.blocks`.
- `tantra ask` uses the same client.

### Phase 4 — web (`tantra-ui-web`)
- BFF `/api/acp` WebSocket proxy to `/api/v1/acp` forwarding `Authorization`, closing upstream on
  `ReadableStream.cancel()`; TS ACP client; model picker in chat settings.

### Phase 5 — Python SDK (`tantra` package)
- `tantra.agent.AgentClient` (async, `websockets` or stdio `tantra-acp`), typed events,
  `model=` parameter; `python -m tantra.agent` REPL. Decimal-only for any numeric payloads.

### Phase 6 — retire bespoke paths
- Remove `/ws/chat` + web `/api/chat`, TUI's direct `run_turn` wiring; close this plan.

## 3. Design-review gate (`.agents/checklists/design-review.md`)
- DAG: `tantra-agent-acp` (L3 agent) → chat/llm/core only; runtime (L3 house) → agent-acp;
  ui (L5) → agent-acp client; Python (L6) → public WS API. No cycles, R1–R4 preserved.
- Interface-first: `ModelCatalog` trait + typed `AcpError`/`CatalogError` before adapters.
- Bounded: every channel bounded; WS frames capped; per-connection session cap.
- Security: `/api/v1/acp` behind `auth_middleware` whenever auth is enabled; sessions scoped by
  `Caller`; BFF forwards `Authorization`; no secrets on the wire; permission prompts
  fail closed (no client reply → reject).
