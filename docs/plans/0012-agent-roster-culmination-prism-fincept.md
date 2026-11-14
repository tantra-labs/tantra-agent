# 0012 — Agent Roster Culmination: Prism Insight (13) + Fincept (37) into the Tantra Persona Desk

> **Status**: Approved Design & Implementation Roadmap (docs/design/plan only — no code landed yet)
> **Target Scope**: `tantra-agent` — persona roster (`crates/tantra-agent-prompt/src/persona.rs`), bundled skills
> (`skills/catalog.json` + `skills/*/SKILL.md`), playbook (`docs/design/agents/0001-guide.md`)
> **Reference Sources (upstream, read-only)**
> - Prism Insight (13 agents): <https://github.com/dragon1086/prism-insight> — AGPL-3.0 AI stock analysis
>   & trading system (KR/US). Agent roster per repo `README.md` § "AI Agent System" and the product's
>   13-agent list (Technical Analyst → Dialogue Manager).
> - Fincept Terminal (37 agents): <https://github.com/Fincept-Corporation/FinceptTerminal> —
>   `fincept-qt/scripts/agents/{EconomicAgents,GeopoliticsAgents,TraderInvestorsAgent}/configs/agent_definitions.json`
>   (6 economic-school + 20 geopolitical + 11 legendary-investor personas, JSON-config "personas are
>   configuration, not code").
> - Existing Tantra roster: `tantra-agent/docs/design/agents/0001-guide.md` (9 personas) and
>   `crates/tantra-agent-prompt/src/persona.rs` (9 `AgentPersona` variants).

---

## 0. Executive Summary

Tantra's current persona desk has **9 code-backed personas**. Two open-source agent ecosystems —
**Prism Insight (13 agents)** and **Fincept Terminal (37 agents)** — demonstrate overlapping, more
fine-grained agent sets for the same job: turn market data into analyst-grade research, a tradeable
strategy, buy/sell decisions, and investor-facing communication.

This plan **culminates all 59 source agents (9 existing + 13 Prism + 37 Fincept) into a clubbed set of
14 Tantra personas**: the 9 existing personas absorb everything they already cover, and **5 net-new
personas** close the genuine coverage gaps (trading flows, industry, news/catalysts, strategy
synthesis, geopolitics). The remaining 37 Fincept agents are folded in as **lenses** (economic-school
lenses, legendary-investor mentor lenses, geopolitical focus modes) rather than as standalone personas.

**Design decisions**
1. **Club, do not multiply.** No persona is added if an existing persona already owns its core job.
   Prism's Technical Analyst / Financial Analyst / Market Analyst / Portfolio Consultant / Buy &
   Sell Specialists / Summary Optimizer / Quality Evaluator / Dialogue Manager are all absorbed by
   existing Tantra personas (see §3 coverage matrix).
2. **Lenses instead of personas.** Fincept's 37 agents are not 37 new personas. They become
   parameterized *lenses* on host personas: 6 economic schools → Macro Strategist, 11 investor
   mentors → Investment Strategist, 20 geopolitics agents → Geopolitics Analyst (region /
   world-order / grand-chessboard focus modes).
3. **Advisory-only & zero-float invariants hold.** Every new persona inherits the Tantra system
   standards (grounded tool data, exact `Decimal` arithmetic, NoFX untrusted-AI boundary, Indian
   market conventions) exactly as the existing 9 do.
4. **One persona set for both research and pipeline roles.** Prism's pipeline roles (buy/sell
   decisions, telegram summaries, QA, dialogue) are treated as *modes of existing personas* so the
   interactive desk (`/agent`, `--persona`) and the automated research pipeline stay one coherent roster.

---

## 1. Source Census

### 1.1 Existing Tantra roster (9) — `tantra-agent`

| # | Persona | Core discipline |
|---|---------|-----------------|
| T1 | TantraAI | Generalist lead; picks tools, lenses and depth |
| T2 | Analyst | Fundamental equity research & forensic financials |
| T3 | Quant | Statistics, factor modeling, backtests |
| T4 | Trader | Price action, levels, tactical risk-reward plans |
| T5 | Portfolio Manager | Allocation, diversification, drift, rebalancing |
| T6 | Risk Manager | Downside-first: drawdowns, stress, sizing, hedges |
| T7 | Macro Strategist | Top-down macro regimes, policy, rates, currencies |
| T8 | Value Investor | Moats, owner earnings, intrinsic value, margin of safety |
| T9 | Devil's Advocate | Thesis stress-testing, bear cases, pre-mortems |

### 1.2 Prism Insight (13) — <https://github.com/dragon1086/prism-insight>

| # | Agent | Focus |
|---|-------|-------|
| P1 | Technical Analyst | Price & Volume Analysis |
| P2 | Trading Flow Analyst | Investor Patterns |
| P3 | Financial Analyst | Valuation & Metrics |
| P4 | Industry Analyst | Business & Competition |
| P5 | Information Analyst | News & Catalysts |
| P6 | Market Analyst | Macro Environment |
| P7 | Investment Strategist | Strategy Synthesis |
| P8 | Summary Optimizer | Telegram Messages |
| P9 | Quality Evaluator | Output Validation |
| P10 | Buy Specialist | Entry Decisions |
| P11 | Sell Specialist | Exit Timing |
| P12 | Portfolio Consultant | User Advice |
| P13 | Dialogue Manager | Conversation Context |

Prism's own grouping (README § "AI Agent System"): **Stock analysis** = P1–P6 base sections;
**Strategy & summary** = P7–P8 (created at run time); **Trading** = P10–P11 (KR/US buy and sell);
**Communication & consultation** = P8/P9/P13 (evaluation, optimization, follow-up).

### 1.3 Fincept Terminal (37) — <https://github.com/Fincept-Corporation/FinceptTerminal>

`fincept-qt/scripts/agents/*/configs/agent_definitions.json` — "personas are configuration, not code";
every entry is `{id, name, description, category, version, capabilities, config{model, instructions,
tools, …}}`. Total = 6 + 20 + 11 = **37** (matches the "37 AI agents" figure on fincept.in).

**Economic schools (6) — `EconomicAgents/configs/agent_definitions.json`**

| # | id | School |
|---|-----|--------|
| F1 | `capitalism_agent` | Capitalism Economic Analyst (free markets, supply-side) |
| F2 | `keynesian_agent` | Keynesian Economic Analyst (aggregate demand, fiscal stabilization) |
| F3 | `neoliberal_agent` | Neoliberal Economic Analyst (deregulation, trade liberalization) |
| F4 | `socialism_agent` | Socialist Economic Analyst (inequality, redistribution, worker ownership) |
| F5 | `mixed_economy_agent` | Mixed Economy Analyst (market-failure vs government-failure pragmatism) |
| F6 | `mercantilist_agent` | Mercantilist Trade Analyst (national strength, strategic industries) |

**Legendary investors (11) — `TraderInvestorsAgent/configs/agent_definitions.json`**

| # | id | Mentor | Discipline |
|---|-----|--------|-----------|
| F7 | `warren_buffett_agent` | Buffett | Moats, owner earnings, capital allocation |
| F8 | `benjamin_graham_agent` | Graham | Deep value, hard quantitative screens, MoS |
| F9 | `peter_lynch_agent` | Lynch | GARP, PEG, six-category classification |
| F10 | `charlie_munger_agent` | Munger | Mental models, inversion, incentives, biases |
| F11 | `seth_klarman_agent` | Klarman | Downside-first, special situations, MoS ≥ 40% |
| F12 | `howard_marks_agent` | Marks | Cycles, second-level thinking, positioning |
| F13 | `joel_greenblatt_agent` | Greenblatt | Magic formula (ROC + earnings yield) |
| F14 | `david_einhorn_agent` | Einhorn | Catalyst-driven value, forensic, long/short |
| F15 | `bill_miller_agent` | Miller | Contrarian value in tech, FCF anchor |
| F16 | `jean_marie_eveillard_agent` | Eveillard | Global value, capital preservation, gold |
| F17 | `marty_whitman_agent` | Whitman | Distressed/credit-first, capital structure, PMV |

**Geopolitics (20) — `GeopoliticsAgents/configs/agent_definitions.json`**

*Prisoners of Geography — regions (10):* Russia, China, USA, Europe, Middle East, Africa,
India–Pakistan, Japan–Korea, Latin America, Arctic.

*World Order (Kissinger) — order schools (5):* American (liberal internationalism), Chinese
(tianxia / tributary), European (Westphalian), Islamic (ummah / caliphate legacy), Multipolar.

*Grand Chessboard (Brzezinski) — concepts (5):* Eurasian Balkans, Geopolitical Pivots, Active
Geostrategic Players, American Primacy, Eurasian Heartland.

---

## 2. Culmination Design — the 14-Persona Tantra Desk

### 2.1 Roster (9 existing + 5 new)

| # | Persona | Source | Notes |
|---|---------|--------|-------|
| T1 | **TantraAI** | existing | Generalist lead. Also owns Prism **Summary Optimizer**, **Quality Evaluator**, **Dialogue Manager** as output/conversation modes. |
| T2 | **Analyst** | existing | Absorbs Prism **Financial Analyst** (P3). |
| T3 | **Quant** | existing | Optionally hosts the Fincept **Greenblatt** (F13) magic-formula screen as a quant lens. |
| T4 | **Trader** | existing | Absorbs Prism **Technical Analyst** (P1), **Buy Specialist** (P10), **Sell Specialist** (P11) as its read → plan → decide sub-steps. |
| T5 | **Portfolio Manager** | existing | Absorbs Prism **Portfolio Consultant** (P12). |
| T6 | **Risk Manager** | existing | Unchanged. |
| T7 | **Macro Strategist** | existing | Absorbs Prism **Market Analyst** (P6). Hosts the 6 Fincept **economic-school lenses** (F1–F6). |
| T8 | **Value Investor** | existing | Unchanged. |
| T9 | **Devil's Advocate** | existing | Unchanged. |
| N1 | **Trading Flow Analyst** | **NEW** | Prism P2 — investor patterns. |
| N2 | **Industry Analyst** | **NEW** | Prism P4 — business & competition. |
| N3 | **Information Analyst** | **NEW** | Prism P5 — news & catalysts. |
| N4 | **Investment Strategist** | **NEW** | Prism P7 — strategy synthesis. Hosts the 11 Fincept **mentor lenses** (F7–F17). |
| N5 | **Geopolitics Analyst** | **NEW** | Fincept 20 — geopolitical environment; region / world-order / chessboard focus modes. |

### 2.2 Coverage matrix — every source maps to exactly one Tantra persona

| Source | Host persona | Treatment |
|--------|--------------|-----------|
| P1 Technical Analyst | Trader | absorbed (existing capability) |
| **P2 Trading Flow Analyst** | **Trading Flow Analyst** | **net-new persona** |
| P3 Financial Analyst | Analyst | absorbed |
| **P4 Industry Analyst** | **Industry Analyst** | **net-new persona** |
| **P5 Information Analyst** | **Information Analyst** | **net-new persona** |
| P6 Market Analyst | Macro Strategist | absorbed |
| **P7 Investment Strategist** | **Investment Strategist** | **net-new persona** |
| P8 Summary Optimizer | TantraAI | absorbed (terse-telegram output mode) |
| P9 Quality Evaluator | TantraAI | absorbed (self-verification mode) |
| P10 Buy Specialist | Trader | absorbed (entry-decision step) |
| P11 Sell Specialist | Trader | absorbed (exit-timing step) |
| P12 Portfolio Consultant | Portfolio Manager | absorbed |
| P13 Dialogue Manager | TantraAI | absorbed (conversation continuity) |
| F1–F6 Economic schools | Macro Strategist | **lens** (school selector) |
| F7–F17 Investor mentors | Investment Strategist | **lens** (mentor selector) |
| F18–F27 PoG regions | Geopolitics Analyst | **lens** (region selector) |
| F28–F32 World orders | Geopolitics Analyst | **lens** (order-school selector) |
| F33–F37 Grand Chessboard | Geopolitics Analyst | **lens** (concept selector) |

---

## 3. New Persona Specifications (design summary)

Full role descriptions, standard operating procedures, answer formats and primary tools for the five
new personas live in the companion design doc:
`tantra-agent/docs/design/agent-roster-culmination.md`. Summary:

1. **Trading Flow Analyst** — reads who is trading and why: delivery % vs. total volume, FII/DII
   flows, open-interest build-up, block/bulk deals, promoter & insider holdings, institutional
   ownership and crowding. Tools: `tantra_ticker_technicals`, `tantra_derivatives_analytics`,
   `tantra_screener`, `tantra_news_search`, `tantra_calculate` (plus the flow data ports planned in
   `tantra-agent` plan 0010 / `tantra-data` gaps 0013–0014).
2. **Industry Analyst** — business & competition: industry structure (Porter's forces), value-chain
   position, market share & share trends, moat verification, competitive intensity, regulatory and
   policy exposure of the sector. Tools: `tantra_ticker_fundamentals`, `tantra_ticker_peers`,
   `tantra_sector_stocks`, `tantra_screener`, `tantra_filings`, `tantra_news_search`.
3. **Information Analyst** — news & catalysts: news flow, filings, earnings/event calendar,
   scheduled catalysts, sentiment, and "what is already priced in" for an instrument. Tools:
   `tantra_news_search`, `tantra_filings`, `tantra_ticker_price`, `tantra_calculate`.
4. **Investment Strategist** — synthesis: takes the lens outputs (technical, flows, fundamental,
   industry, news, macro) and produces one coherent investment thesis: view, conviction, sizing,
   entry/exit framework, key assumptions, and kill criteria. Hosts the 11 mentor lenses (Buffett /
   Graham / Lynch / Munger / Klarman / Marks / Greenblatt / Einhorn / Miller / Eveillard / Whitman)
   as first-class "style" modifiers that re-weight the thesis according to each mentor's discipline.
   Tools: everything the desk has (`tantra_*`), `tantra_backtest_run` for verification.
5. **Geopolitics Analyst** — geopolitical environment as a market input: named-terrain/constraint
   analysis (regions), contested-legitimacy analysis (world orders), great-power/chessboard analysis
   (concepts) — each with a concrete transmission map to asset classes, sectors, currencies and
   commodities. Tools: `tantra_macro_series`, `tantra_news_search`, `tantra_ticker_price`,
   `tantra_calculate`, `tantra_web_search`.

---

## 4. Implementation Phases (when approved — this plan lands docs only)

> **Current status**: Phase 0 only. No code, skill, or playbook changes are part of this plan's landing.

- **Phase 0 (this plan)**: census, coverage matrix, 14-persona design, lens design, and the plan
  document itself. Companion design doc:
  `docs/design/agent-roster-culmination.md`.
- **Phase 1 — code-backed roster** (`tantra-agent-prompt`): add 5 new `AgentPersona` variants
  (`TradingFlowAnalyst`, `IndustryAnalyst`, `InformationAnalyst`, `InvestmentStrategist`,
  `GeopoliticsAnalyst`) with `name/title/tagline/primary_tools/system_prompt`, extend `from_name`
  with aliases (e.g. `trading-flow`, `flow`, `industry`, `news`, `information`, `strategist`,
  `geopolitics`, `geo`; plus folded-source aliases `technical-analyst → trader`,
  `financial-analyst → analyst`, `market-analyst → macro-strategist`,
  `portfolio-consultant → portfolio-manager`, `buy → trader`, `sell → trader`, `summary → tantra-ai`,
  `quality → tantra-ai`, `dialogue → tantra-ai`), and bump `all()` to 14. Tests in
  `crates/tantra-agent-prompt/tests/unit_tests.rs` (count 9 → 14 + new `from_name` cases) per the
  parallel `tests/` rule.
- **Phase 2 — skills**: `skills/{trading-flow-analyst,industry-analyst,information-analyst,
  investment-strategist,geopolitics-analyst}/SKILL.md` + 5 `catalog.json` entries; update
  `docs/design/agents/0001-guide.md` persona table (9 → 14) and per-persona design docs
  (`docs/design/agents/*.md`).
- **Phase 3 — lenses**: represent the 37 folded Fincept agents as parameterized lens presets
  (economic-school selector on Macro Strategist; mentor selector on Investment Strategist; region /
  world-order / chessboard selectors on Geopolitics Analyst). Deliverable form to be decided at
  Phase 3 kickoff: a `Lens` enum in `tantra-agent-prompt` and/or skill-preset files. Each lens is
  documented in `docs/design/agent-roster-culmination.md` §4.
- **Phase 4 — surfacing**: `/agent` command list, `--persona` help text, and the TUI/CLI reference
  (`tantra-ui/docs/reference/cli-and-tui.md`) gain the new names; count-pin tests updated. Full UI
  wiring (persona + lens selectors, `/desk` runner, and the winning-trade flow) is specified and
  tracked by `tantra-ui/docs/plans/0007-agent-desk-ui-wiring-and-winning-trade-flow.md` + its design
  doc `tantra-ui/docs/design/agent-desk-ui-and-winning-trade-flow.md`.

---

## 5. Invariants & Constraints (binding)

- **Advisory Only / NoFX**: none of the new personas issues un-gated execution directives; all
  trading proposals pass the `Decision → DecisionValidator → RiskEngine` pipeline.
- **Grounded Data**: every figure must come from this turn's tool results; no memory-prices.
- **Zero Floats**: exact `rust_decimal::Decimal` for all financial quantities.
- **Indian market grounding**: NSE/BSE/MCX/NFO conventions, SEBI regulations, circuit limits, IST
  hours — alongside the global benchmarks the Geopolitics and Macro personas need.
- **Zero recon / no code**: this plan lands documentation only; Phases 1–4 require explicit go-ahead.

---

## 6. Cleanup & Tracking

- When Phases 1–4 land, this plan is closed in `docs/plans/README.md` (Implemented table) and the
  completed parts are removed from `docs/design/` scratch space if any.
- Sources are upstream read-only references; **no changes are pushed to** the Prism Insight or
  FinceptTerminal repositories.
