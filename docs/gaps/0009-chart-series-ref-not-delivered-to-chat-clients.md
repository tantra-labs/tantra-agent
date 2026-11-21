# 0009 — `DisplayBlock::Chart` series refs are not resolvable by chat clients

**Status**: open

## What's stubbed or hardcoded
`DisplayBlock::Chart { title, chart_type, series_ref }` carries only a
`$tantra/ref/<uuid>` URI into the ephemeral `tantra-agent-core::ref_store`. Chat
clients consuming `SessionEvent::ToolFinished { blocks, .. }` (the tantra-ui TUI
analysis page, `tantra-ui/crates/tantra-tui`) have no handle on that store, so a
Chart card renders its title plus an explicit "series unavailable" empty state
instead of the plotted series.

## Why
The ref store is process-local to the tool host and only reachable through the
MCP `read_reference` tool; `run_turn` does not resolve refs, and `ToolFinished`
ships block metadata only.

## How to close it
Either (a) have `run_turn` resolve `Chart` refs on the success path and attach a
bounded, decimal-string series (`Vec<(i64, String)>`, capped e.g. 500 points) to
the block/event, or (b) expose a `RefReader` port on `tantra-agent-chat` that a
client passes in. Then the TUI's Chart card plots the series and the stand-in
comment citing this gap is removed.
