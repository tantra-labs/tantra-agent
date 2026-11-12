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

            // Rule R1: No tantra-agent-* crate depends on tantra-runtime-*, tantra-exec-*, or tantra-market-*
            assert!(
                !dep_name.starts_with("tantra-runtime"),
                "Violation of Rule R1: `{pkg_name}` cannot depend on `{dep_name}`"
            );
            assert!(
                !dep_name.starts_with("tantra-exec"),
                "Violation of Rule R1: `{pkg_name}` cannot depend on `{dep_name}`"
            );
            assert!(
                !dep_name.starts_with("tantra-market"),
                "Violation of Rule R1: `{pkg_name}` cannot depend on `{dep_name}`"
            );

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

            // Rule R4: The only downward edge out of Tier 3A to tantra-ledger is tantra-agent-chat-tools
            if dep_name.starts_with("tantra-ledger") {
                assert_eq!(
                    pkg_name, "tantra-agent-chat-tools",
                    "Violation of Rule R4: only tantra-agent-chat-tools may depend on tantra-ledger, found `{pkg_name}`"
                );
            }
        }
    }
}
