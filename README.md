# व्यापार・代理 tantra-agent

> *AI Agent & Autonomous Research Engine for the Tantra Platform.*

Autonomous quantitative research, LLM decision reasoning, prompt replay cassettes, Model Context Protocol (MCP) server, and genetic strategy synthesis.

## Architecture & Subcrates

- `tantra-agent` — Multi-step reasoning agent state machine & facade
- `tantra-agent-core` — Shared AI traits (`LlmProvider`, `PromptBuilder`, `DecisionValidator`), grounding validators
- `tantra-agent-llm` — Cloud & local LLM client adapters (Anthropic Claude, OpenAI, Gemini, Ollama)
- `tantra-agent-prompt` — Versioned prompt templates & context serializers
- `tantra-agent-replay` — SHA-256 prompt/completion cassette store for deterministic backtest replay
- `tantra-agent-trader-loop` — Event-driven trader loop wiring LLM reasoning to `tantra-risk`
- `tantra-agent-research` — Autonomous hypothesis generation & walk-forward research loop
- `tantra-agent-analytics` — Reasoning attribution, Deflated Sharpe Ratio (DSR), and performance analytics
- `tantra-agent-genetic` — StockSharp Odysseus genetic strategy synthesizer & parameter mutator
- `tantra-agent-mcp` — Model Context Protocol (`rmcp`) server & tool endpoints
- `tantra-agent-tools` — Typed `AgentTool` registry for screener, depth, and portfolio queries
- `tantra-agent-plugins` — Extensible plugin manifest & capability discovery
- `tantra-agent-report` — Embedded Typst / Markdown tearsheet & research report compiler

## Invariants

- **Untrusted AI Principle**: LLMs are treated strictly as untrusted decision proposers; all completions are validated into `tantra_core::Decision` and gated by `tantra_risk::RiskEngine`.
- **Deterministic Replay**: Every prompt, completion, seed, and temperature is SHA-256 cassette hashed.
- **Strict Decimals**: Zero floating-point arithmetic across all calculations.

## Build & Test

```bash
make prod
make check
make test
```

License: AGPL-3.0.
