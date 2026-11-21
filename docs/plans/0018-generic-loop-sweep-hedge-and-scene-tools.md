# 0018 — Generic Strategy Loop, Sweep, Hedge & Scene Tools

**Status**: Implemented 2026-11-21 (agent side; runtime chat wiring + tantra-ui OverlayScene block render tracked in follow-ups)

## 0. Executive Summary & Objective

Expose the runtime's generic strategy loop to chat and MCP clients as four `ToolRisk::ReadOnly`
tools: `strategy_loop_eval`, `strategy_loop_sweep`, `strategy_hedge_recommend` (proxies over
`/api/v2/studies/strategy-loop/{evaluate,sweep,recommend-hedge}`) and the pure
`chart_scene_annotate`. Root plan 0015 Track T21.6 maps here.

## 1. Context & Gaps Covered

- Agents previously had no typed way to run the runtime strategy loop or hand an overlay scene to
  a UI. Numbers must come from the runtime; tool descriptions state results are untrusted data.
- Follow-ups (not in this plan): runtime chat wiring of `StrategyLoopPort` into the chat tool
  registry; `tantra-ui` rendering of `DisplayBlock::OverlayScene` ("Render on Chart").

## 2. Component Specifications & Phases

- **Port** (`tantra-agent-core::strategy_loop`): `StrategyLoopPort` (evaluate / sweep /
  recommend_hedge), typed `StrategyLoopPortError`, `deny_unknown_fields` request DTOs with
  `validate()` (dates, non-blank ids, positive decimals, sweep grids <= 16 values and <= 64
  candidates). Enum-like fields stay `String`; decimals use `Decimal`.
- **`DisplayBlock::OverlayScene`** (`"type":"overlay_scene"`): title, symbol, timeframe, scene,
  optional unix-second `time_axis`. `scene_annotation()` packages a scene (six required arrays)
  into a `MetricGrid` + `OverlayScene` (+ optional info `Alert`) envelope.
- **Chat tools** (`tantra-agent-chat-tools::strategy_loop`): `create_strategy_loop_tools` /
  `register_strategy_loop_tools` (not part of the pinned read-only catalogue count).
- **MCP** (`tantra-agent-mcp`): 4 tool definitions, `McpServer::with_loop_port`,
  `RuntimeLibraryClient` implements `StrategyLoopPort`; `mcp_stdio_server` attaches it.
