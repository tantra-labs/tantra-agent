# 0020 — present_view: agent-composed pane layouts

**Status**: Implemented 2026-11-21 (2ca5036)

## 0. Executive Summary & Objective

Let the LLM decide what the client's right-hand pane shows. After gathering data with other
tools, the model calls `present_view` once. The tool is pure and read-only and returns a
`tantra/result/v1` envelope with one `DisplayBlock::View` (kind, title, symbols, markdown
summary, up to 12 highlight metrics, `include_tools`). The view references earlier tool results
by tool name, so the model never re-types data.

## 1. Context & Gaps Covered

Serves the TUI dynamic right pane (`tantra-ui`) and the runtime ACP host (agent plan 0019
Phases 2-3). `tantra-ui` currently renders `DisplayBlock::View` minimally (title, kind, summary);
the real pane is built separately.

## 2. Component Specifications & Phases

- `tantra-agent-core`: `ViewKind`, `DisplayBlock::View`, `view::{PresentViewRequest, present_view, ViewError}`
  with caps (title 120, summary 4000, 12 highlights, 4 symbols, 16 tools, 40 chars per short field;
  tool names `[a-z0-9_]+`).
- `tantra-agent-chat-tools`: `PresentViewTool`, `PRESENT_VIEW_GUIDANCE`, `create_presentation_tools()`,
  `register_presentation_tools()`. Hosts register the bundle and append the guidance to their system prompt.
- `tantra-agent-mcp`: `present_view` definition (closed schema) and pure dispatch.
