# 0018 — Genetic synthesizer emits Python by interpolating unvalidated strings; Typst/Markdown reports interpolate raw model text

**Status**: open
**Filed**: 2026-11-23 — workspace design review (root plan 0021) · **Kind**: design-flaw · **Severity**: medium
**Plan**: [0021](../plans/0021-design-review-2026-11.md)

## What's stubbed or hardcoded

- `crates/tantra-agent-genetic/src/synthesizer.rs:287-300,362-410`: `compile_to_python` is `pub` and formats `genome.name` (`class {class_name}`, `self.name = "{}"`), `t.indicator`, `fast_indicator` and price-field names directly into Python source. In `ast.rs:75,84,318` those fields are `String`, and `StrategyGenome: Deserialize` (`crates/tantra-agent-genetic/src/lib.rs:57-106`). `name.replace('-', "_")` is the only sanitation. Only tests call it today, but it is the declared codegen path.
- `crates/tantra-agent-report/src/lib.rs:109-130` and `crates/tantra-agent-report/src/notes.rs:144-175`: `to_typst()` interpolates `title`, `hypothesis`, `entry_rule`, `metrics[].value` and `evidence_refs` unescaped. In Typst, `#read("/etc/...")`, `#image(...)` and `#include` are code-mode functions, so model-authored note text rendered by a Typst compiler can read local files.
- `crates/tantra-agent-research/src/verify/mod.rs:200-225`: the "NoFX safety" gate is a lowercase substring blacklist (`"std::fs::"`, `"reqwest"`, ...). It is bypassed by `use std::fs as f;`, `std ::fs`, `include_str!`, aliased `Command` or Python `__import__`.

## Why

`.agents/rules/security-and-secrets.md` §4.2 forbids interpolating untrusted text into generated source. When LLM-produced strategies or reports are compiled or executed (root plan 0019 §4.3 wires these crates), these become code-execution and file-read primitives, and the verifier gives false assurance.

## How to close it

1. Validate identifiers at construction: an `IndicatorName` newtype backed by the library catalog allow-list and `StrategyName = [A-Za-z_][A-Za-z0-9_]{0,63}`, with `repr`-style escaping. Preferably emit the declarative manifest / `StrategySpec` JSON the runtime already consumes and drop Python codegen.
2. Add `typst_escape()` for all interpolations and render with a `--root` sandbox and no world access.
3. Replace the substring scanner with an AST check (`syn`) or a manifest allow-list.

**Verification**: proptest in `crates/tantra-agent-genetic/tests/` on arbitrary strings in name/indicator asserting the emitted source parses as intended with no extra statements; Typst golden test in `crates/tantra-agent-report/tests/` with a `#read("/etc/passwd")` payload; verifier tests for the bypass strings above; `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `make check`, targeted `cargo test -p tantra-agent-genetic --profile prod`, `make prod`.

## Related

- Root plan 0019 P0.7 covers only the `honba` header string in `synthesizer.rs`.
- Deployment prerequisite: any executor of generated Python needs its own sandbox.
