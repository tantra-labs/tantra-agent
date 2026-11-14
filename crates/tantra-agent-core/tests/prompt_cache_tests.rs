//! Regression tests for prompt cache-hit / cache-miss behaviour (gap 0007).
//!
//! KV prompt caching depends on the system prefix being byte-stable while the
//! turn context changes. These tests pin:
//! - deterministic SHA-256 hashing of identical prompts (cache hit),
//! - distinct hashes for differing prompts (cache miss),
//! - prefix stability under turn-context-only changes (KV cache hit),
//! - prefix invalidation when a prefix section itself changes (cache miss).

use tantra_agent_core::{
    Placement, PromptHash, PromptSection, compute_prompt_hash, default_assemble, hash_prompt_hex,
};

#[test]
fn same_prompt_hashes_identically() {
    let prompt = "System prompt with stable memory profile prefix";
    assert_eq!(compute_prompt_hash(prompt), compute_prompt_hash(prompt));
    assert_eq!(
        PromptHash::compute(prompt),
        PromptHash::compute(prompt),
        "identical prompt must produce identical PromptHash (cache hit)"
    );
    assert_eq!(
        hash_prompt_hex(prompt),
        PromptHash::compute(prompt).to_hex(),
        "hex hash must be identical for identical prompts"
    );
}

#[test]
fn different_prompt_hashes_differently() {
    assert_ne!(
        compute_prompt_hash("alpha prompt"),
        compute_prompt_hash("beta prompt"),
        "differing prompts must produce differing raw digests (cache miss)"
    );
    assert_ne!(
        hash_prompt_hex("alpha prompt"),
        hash_prompt_hex("beta prompt")
    );
    assert_ne!(
        PromptHash::compute("alpha prompt").to_hex(),
        PromptHash::compute("beta prompt").to_hex()
    );
}

#[test]
fn cached_prefix_stable_under_turn_context_change() {
    let base = "System prompt";
    let frozen = PromptSection::new("p1", Placement::CachedPrefix, "frozen instructions");

    let a1 = default_assemble(
        base,
        &[
            frozen.clone(),
            PromptSection::new("t1", Placement::TurnContext, "user turn A"),
        ],
    );
    let a2 = default_assemble(
        base,
        &[
            frozen,
            PromptSection::new("t1", Placement::TurnContext, "user turn B"),
        ],
    );

    // KV cache hit on the prefix: only the turn context changed.
    assert_eq!(
        a1.cached_prefix, a2.cached_prefix,
        "cached prefix must be byte-identical when only turn context changes"
    );
    // The turn context itself is a cache miss and must differ.
    assert_ne!(a1.turn_context, a2.turn_context);
}

#[test]
fn cached_prefix_changes_when_prefix_section_changes() {
    let base = "System prompt";
    let a1 = default_assemble(
        base,
        &[PromptSection::new(
            "p1",
            Placement::CachedPrefix,
            "frozen instructions",
        )],
    );
    let a2 = default_assemble(
        base,
        &[PromptSection::new(
            "p1",
            Placement::CachedPrefix,
            "updated instructions",
        )],
    );

    assert_ne!(
        a1.cached_prefix, a2.cached_prefix,
        "changing a cached-prefix section must invalidate the prefix (cache miss)"
    );
}

#[test]
fn prompt_hash_matches_hex_roundtrip() {
    let prompt = "user turn with fresh content";
    assert_eq!(
        hash_prompt_hex(prompt),
        PromptHash::compute(prompt).to_hex(),
        "raw hex hash and PromptHash hex must agree"
    );
    assert_eq!(
        hash_prompt_hex(prompt),
        PromptHash::compute(prompt).to_string(),
        "Display must render the same hex as the raw helper"
    );
}
