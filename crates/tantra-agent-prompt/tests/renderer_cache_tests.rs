//! Regression tests for the prompt context renderers used by the persistent
//! memory layer: stable cached-prefix sections and per-turn recall sections.

use chrono::Utc;

use tantra_agent_core::{
    MemoryEntry, MemoryKind, MemoryOrigin, Placement, ScoredMemory, SessionId,
};
use tantra_agent_prompt::{render_memory_profile_section, render_turn_recalled_memory};

/// Build a minimal [`MemoryEntry`] fixture.
fn entry(kind: MemoryKind, content: &str) -> MemoryEntry {
    MemoryEntry {
        id: 1,
        kind,
        content: content.to_string(),
        tags: vec![],
        importance: 3,
        pinned: false,
        origin: MemoryOrigin::User,
        source_session: None,
        created_at: Utc::now(),
        expires_at: None,
    }
}

#[test]
fn memory_profile_section_is_stable_and_cached() {
    let entries = vec![entry(MemoryKind::Preference, "prefers small-cap momentum")];

    let s1 = render_memory_profile_section(&entries, 1);
    let s2 = render_memory_profile_section(&entries, 1);

    // Byte-identical output => the section can live in the KV-cached prefix.
    assert_eq!(s1.text, s2.text);
    assert_eq!(s1.id, "02_memory_profile");
    assert_eq!(s1.placement, Placement::CachedPrefix);
}

#[test]
fn memory_profile_section_orders_by_kind() {
    let entries = vec![
        entry(MemoryKind::Instruction, "Never trade F&O"),
        entry(MemoryKind::Preference, "avoids PSU banks"),
        entry(MemoryKind::Fact, "NIFTY weekly options expire Thursday"),
    ];

    let sec = render_memory_profile_section(&entries, 10);

    assert_eq!(sec.placement, Placement::CachedPrefix);
    assert!(sec.text.contains("Standing Instruction: Never trade F&O"));
    assert!(sec.text.contains("Preference: avoids PSU banks"));
    assert!(
        sec.text
            .contains("Fact: NIFTY weekly options expire Thursday")
    );
}

#[test]
fn empty_profile_with_total_shows_hint() {
    let sec = render_memory_profile_section(&[], 5);

    assert_eq!(sec.placement, Placement::CachedPrefix);
    assert!(sec.text.contains("memories recorded"));
    assert!(sec.text.contains("memory_search"));
}

#[test]
fn turn_recalled_memory_placement_and_content() {
    let recalled = vec![ScoredMemory {
        entry: MemoryEntry {
            id: 7,
            kind: MemoryKind::Fact,
            content: "prefers weekly options on Thursdays".to_string(),
            tags: vec![],
            importance: 2,
            pinned: false,
            origin: MemoryOrigin::Model,
            source_session: Some(SessionId::new()),
            created_at: Utc::now(),
            expires_at: None,
        },
        score: 0.9,
    }];

    let sec = render_turn_recalled_memory(&recalled);

    assert_eq!(sec.id, "turn_recalled_memory");
    assert_eq!(sec.placement, Placement::TurnContext);
    assert!(sec.text.contains("prefers weekly options on Thursdays"));
}
