# `tantra-agent` — Migration Guardrails & Invariants

Mandatory architectural guardrails for the `honba-nou → tantra-agent` migration and for all
subsequent work in this repository.

## 1. Eliminate Upward Dependency on Runtime & Exec (R1–R3)

- In `honba-nou/Cargo.toml`, `honba-nou-strategy` pointed to `../honba-ya/crates/honba-ya-strategy`. In `tantra-agent`, all dependencies strictly flow downward (`tantra-core`, `tantra-risk`).
- All external storage and reader capabilities are inverted via traits in `tantra-agent-core::ports` and `tantra-agent-chat-tools::ports`.

## 2. Zero Floating Point Math

- All financial figures, quantities, prices, metrics, and drawdown numbers strictly use `rust_decimal::Decimal`.
- Internal return/statistics kernels in `tantra-agent-analytics`, `-forecast`, and `-panel` compute in `f64` without crossing the crate boundary (documented per-crate); no money/price/quantity arithmetic ever crosses as a float.

## 3. No Inline Tests in Production Source

- The rule: zero `#[cfg(test)]` blocks in `crates/*/src/`; all tests reside in `crates/*/tests/`.
- **Current state**: two violations remain —
  - `crates/tantra-agent-core/src/ref_store.rs` (inline `mod tests`)
  - `crates/tantra-agent-research/src/verify/mod.rs` (inline `mod verify_unit_tests`)
- Both are tracked as **gap 0001** and planned in **plan 0008**. Until closed, the grep below
  is expected to report those two files; the target is zero matches.

```bash
grep -rn "#\[cfg(test)\]" crates/*/src/ || true
```

## 4. Architectural Dependency Rules (R1–R5)

Enforced by integration test `crates/tantra-agent-chat/tests/dependency_rules.rs` inspecting `cargo metadata`:

| Rule | Statement |
|------|-----------|
| R1 | No `tantra-agent-*` crate may depend on `tantra-runtime-*`. Runtime depends on agent or implements agent ports. |
| R2 | No `tantra-agent-*` crate may depend on `tantra-exec-*`. Execution is mediated by runtime and risk engines. |
| R3 | No `tantra-agent-*` crate may depend on `tantra-market-*`. Market reads are inverted through reader traits. |
| R4 | No `tantra-agent-*` crate may depend on `tantra-ledger` except `tantra-agent-chat-tools` (types only). |
| R5 | External consumers/adapters depend on minimal core traits (`tantra-agent-core::ports`). |

## 5. Untrusted AI Principle (NoFX)

- LLMs are untrusted signal proposers. Chat turns never produce a trade or bypass the `DecisionParser → PriceBand → RiskEngine` pipeline.
- Every decision is parsed into `tantra_core::Decision` (`#[serde(deny_unknown_fields)]`) and clamped before reaching execution buffers.

## Verification Commands

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace --profile prod
```
