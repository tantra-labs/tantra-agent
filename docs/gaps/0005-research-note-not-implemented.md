# Gap 0005 — `ResearchNote` in `tantra-agent-report` Not Implemented

## What's Missing

`tantra-agent-report` implements `TearsheetReport`, `Metric`, `ReportSection`, and a
Typst/Markdown source generator consuming `tantra-core` `Decimal` metrics (plus LLM
narrative narration behind the `llm` feature). Roadmap Phase E lists `ResearchNote` in
`tantra-agent-report` as planned, but it does not exist.

There is no structured research-note type for capturing hypothesis, evidence, and
verification outcome in a renderable report format.

## Why It Matters

Without `ResearchNote`, the autonomous research loop (`tantra-agent-research`) cannot emit
structured, renderable research notes tying a generated hypothesis to its walk-forward
verification result and DSR/PBO statistics.

## How to Close

1. Define a `ResearchNote` type in `tantra-agent-report` (hypothesis, evidence refs,
   verification outcome, metrics).
2. Integrate with `tantra-agent-research` output so verified hypotheses produce
   `ResearchNote`s.
3. Add Typst/Markdown rendering for research notes.
4. Add unit tests in `crates/tantra-agent-report/tests/`.

See `docs/plans/0009-persistent-memory-mcp-client-and-phase-e.md` for the tracking plan.
