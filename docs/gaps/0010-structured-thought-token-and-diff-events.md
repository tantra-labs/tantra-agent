# 0010 — Emit structured thought deltas, token usage, raw tool payloads, and strategy diff blocks

**Status**: open

## What's stubbed or hardcoded
Consumer: `tantra-ui-web` (`AgentPane`, `AgentRunTrace`, `ToolPayloadTree`, `StrategyDiffCard` under root plan `0020`):
- `SessionEvent` / `ChatStreamEvent` in `tantra-agent-chat` emits assistant text chunks, tool call start/completion summaries, and `DisplayBlock` items, but does not expose:
  1. `ThoughtDelta { text }` — streamed chain-of-thought / planning progress for live reasoning trace rows.
  2. `TokenUsage { total_tokens, prompt_tokens, completion_tokens }` — exact token telemetry from `tantra-agent-llm` responses (`AgentRunRecord.nativeUsage` in `tantra-ui-web` currently falls back to `chars / 4` character-ratio estimates).
  3. Raw JSON arguments and results (`raw_input` / `raw_output`) alongside display summaries for expandable tree inspection (`ToolPayloadTree`).
  4. Structured strategy diff metadata (`lang`, `title`, hunk headers/lines) for interactive hunk accept/reject before applying to Python/Pine editors (`StrategyDiffCard`).

## Why
Root plan `0020` scope was restricted to `tantra-ui-web/`. `tantra-ui-web`'s `AgentPane.tsx` already handles optional `thought_delta`, `token_usage`, `raw_input`, and `raw_output` fields forward-compatibly and parses Markdown `diff` / Python / Pine blocks when structured diff metadata is absent.

## How to close it
1. Extend `tantra-agent-llm` response metadata to surface provider token counts (`prompt_tokens`, `completion_tokens`, `total_tokens`) and reasoning deltas where supported.
2. Add optional `thought_delta`, `token_usage`, `raw_input`, `raw_output`, and structured diff fields to `SessionEvent` in `tantra-agent-chat` (backward-compatible `#[serde(default, skip_serializing_if)]`).
3. Coordinate with `tantra-runtime/docs/gaps/0014-chat-trace-streaming-and-thread-persistence.md` so `/ws/chat` forwards these events to `tantra-ui-web`.
