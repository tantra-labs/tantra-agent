# 0015 — FIRE Planning Chat & MCP Tools (`fire_simulate`, `fire_portfolio_check`)

**Status**: Proposed (2026-11-18) — Track **T19.3**

Master plan: root [`docs/plans/0013-fire-planning-and-cashflow-platform.md`](../../../docs/plans/0013-fire-planning-and-cashflow-platform.md).
Depends on: [`tantra-ledger` plan 0006](../../../tantra-ledger/docs/plans/0006-fire-bucket-glidepath-and-swr-metrics.md), [`tantra-runtime` plan 0008](../../../tantra-runtime/docs/plans/0008-fire-planning-persistence-and-rest-api.md).

---

## 0. Executive Summary & Objective

Enable the AI Research Assistant & MCP gateway to run deterministic, Decimal-exact FIRE runway simulations, Coast/Barista comparisons, Sequence-of-Returns stress checks, and live portfolio-hydrated retirement checks while preserving every Agent dependency rule (**R1–R4** in `tantra-agent-chat/tests/dependency_rules.rs`) and `ToolTraits` safety constraint — with zero changes to `tantra-core` or `tantra-agent-tools` dependency boundaries.

---

## 1. Architecture & Dependency Rules Compliance (R1–R4)

Rule **R4** explicitly permits `tantra-agent-chat-tools` → `tantra-ledger-*` and `tantra-agent-mcp` → `tantra-ledger-allocation`. Hosting FIRE calculation in `tantra-ledger-allocation::fire` preserves all existing rules with zero modifications to `dependency_rules.rs`:

1. **`tantra-agent-chat-tools` (R2/R4 ReadOnly Chat Tools, zero `tantra-runtime-*` / `tantra-market-*`)**:
   - Add `FireSimulateTool` (`fire_simulate`) and `FirePortfolioCheckTool` (`fire_portfolio_check`) delegating pure calculation directly to `tantra_ledger_allocation::fire::{simulate_fire_runway, calculate_fire_archetypes, simulate_post_fire_cashflow, run_sor_stress}`:
     - Closed `#[serde(deny_unknown_fields)]` `JsonSchema` input matching `FirePlanConfig`.
     - `ToolRisk::ReadOnly` (explicit planning + portfolio/tax projection, never places orders).
     - `fire_portfolio_check` queries injected `Arc<dyn PortfolioReader>` + optional `Arc<dyn TaxReportReader>` to pre-populate `current_corpus`, equity/cash split, and Finance Act 2025 capital-gains tax drag.
   - Register in the ReadOnly tool catalog and update the catalogue count assertion in `tests/unit_tests.rs`.
2. **`tantra-agent-mcp` (R1/R4 MCP Gateway)**:
   - Uses its already-permitted `tantra-ledger-allocation` edge to expose `tantra_fire_simulate` and `tantra_fire_portfolio_check` wrapped in `TantraResultEnvelope` (`tantra/result/v1`) with structured `DisplayBlock` metric grid + projection table.
3. **`tantra-agent-prompt`**:
   - Add domain grounding notes for Indian SWR (`30×` / `33×` vs `25×`), Coast/Barista FIRE, and Risk-Gated Pillar 2 theta cashflow.

---

## 2. Tests & Verification

- `crates/tantra-agent-chat/tests/dependency_rules.rs`: verify R1–R4 remain 100% green (`cargo metadata` graph check).
- `crates/tantra-agent-chat-tools/tests/fire_tools_tests.rs`: golden schema validation, `deny_unknown_fields` rejection, live mock `PortfolioReader` hydration, and deterministic `Decimal` serialization.
- Gate: `cargo fmt --check && cargo clippy --workspace --all-targets --profile prod -- -D warnings && make prod`.
