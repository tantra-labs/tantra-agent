//! Architectural Tier and Dependency Rule verification (§3.2, Rules R1–R5).

use std::process::Command;

#[test]
fn test_architectural_dependency_rules() {
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .output()
        .expect("failed to execute cargo metadata");

    assert!(output.status.success(), "cargo metadata must succeed");

    let metadata: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("valid json metadata");

    let packages = metadata["packages"]
        .as_array()
        .expect("packages array in metadata");

    for pkg in packages {
        let pkg_name = pkg["name"].as_str().unwrap_or("");
        if !pkg_name.starts_with("tantra-agent") {
            continue;
        }

        let deps = pkg["dependencies"].as_array().expect("dependencies array");

        for dep in deps {
            let dep_name = dep["name"].as_str().unwrap_or("");

            // Rule R1: No tantra-agent-* crate depends on tantra-runtime-*, tantra-exec-*, or
            // tantra-market-* — with one compute-only exception: `tantra-agent-mcp` →
            // `tantra-market-pricing` (the derivatives/greeks MCP gateway tools, plan 0007
            // Track 1 Phase 3 — widened 2026-11-14 per architecture.md §2). Market *data*
            // access remains inverted through reader traits; only pure computation is allowed.
            assert!(
                !dep_name.starts_with("tantra-runtime"),
                "Violation of Rule R1: `{pkg_name}` cannot depend on `{dep_name}`"
            );
            assert!(
                !dep_name.starts_with("tantra-exec"),
                "Violation of Rule R1: `{pkg_name}` cannot depend on `{dep_name}`"
            );
            if dep_name.starts_with("tantra-market") {
                // The only allowed market edge: `tantra-agent-mcp` →
                // `tantra-market-pricing` (pure computation, no data access).
                let allowed = pkg_name == "tantra-agent-mcp" && dep_name == "tantra-market-pricing";
                assert!(
                    allowed,
                    "Violation of Rule R1: `{pkg_name}` cannot depend on `{dep_name}`"
                );
            }

            // Rule R2: tantra-agent-chat and tantra-agent-chat-tools do not depend on tantra-agent-trader-loop
            if pkg_name == "tantra-agent-chat" || pkg_name == "tantra-agent-chat-tools" {
                assert_ne!(
                    dep_name, "tantra-agent-trader-loop",
                    "Violation of Rule R2: {pkg_name} cannot depend on tantra-agent-trader-loop"
                );
            }
            if pkg_name == "tantra-agent-trader-loop" {
                assert_ne!(
                    dep_name, "tantra-agent-chat",
                    "Violation of Rule R2: tantra-agent-trader-loop cannot depend on tantra-agent-chat"
                );
                assert_ne!(
                    dep_name, "tantra-agent-chat-tools",
                    "Violation of Rule R2: tantra-agent-trader-loop cannot depend on tantra-agent-chat-tools"
                );
            }

            // Rule R3: tantra-agent-tools has no tokio, async-trait, reqwest, sqlx
            if pkg_name == "tantra-agent-tools" {
                assert_ne!(
                    dep_name, "tokio",
                    "Rule R3 violation: tantra-agent-tools must stay pure"
                );
                assert_ne!(
                    dep_name, "async-trait",
                    "Rule R3 violation: tantra-agent-tools must stay pure"
                );
                assert_ne!(
                    dep_name, "reqwest",
                    "Rule R3 violation: tantra-agent-tools must stay pure"
                );
                assert_ne!(
                    dep_name, "sqlx",
                    "Rule R3 violation: tantra-agent-tools must stay pure"
                );
            }

            // Rule R4: The only downward edges out of Tier 3A to tantra-ledger are
            // tantra-agent-chat-tools (types only) and tantra-agent-mcp →
            // tantra-ledger-allocation (the `tantra_multi_period_optimize` MCP gateway tool,
            // plan 0007 Track 4 — widened 2026-11-14 per the documented decision in
            // `tantra-agent/docs/design/architecture.md` §2).
            if dep_name.starts_with("tantra-ledger") {
                let allowed = pkg_name == "tantra-agent-chat-tools"
                    || (pkg_name == "tantra-agent-mcp" && dep_name == "tantra-ledger-allocation");
                assert!(
                    allowed,
                    "Violation of Rule R4: only tantra-agent-chat-tools (types only) and tantra-agent-mcp → tantra-ledger-allocation may depend on tantra-ledger, found `{pkg_name}` → `{dep_name}`"
                );
            }
        }
    }
}
