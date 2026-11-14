//! Integration tests for the persistent [`UserMemory`] layer (T6.1): per-profile
//! recall budgeters and cache-stable prompt section renderers.
//!
//! A test double implements the [`MemoryStore`] port so the layer stays free of
//! concrete storage (mirroring `tantra-agent-core/tests/memory_tests.rs`).

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use chrono::Utc;
use rust_decimal::Decimal;

use tantra_agent::UserMemory;
use tantra_agent_core::Placement;
use tantra_agent_core::memory::MemoryCategory;
use tantra_agent_core::ports::{
    MemoryEntry, MemoryId, MemoryKind, MemoryOrigin, MemoryProfile, MemoryQuery, MemoryStore,
    NewMemory, ProfileBudget, SaveOutcome, ScoredMemory, StoreError,
};

/// In-memory test double implementing the [`MemoryStore`] port.
#[derive(Default)]
struct TestStore {
    memories: RwLock<HashMap<MemoryId, MemoryEntry>>,
    next_id: RwLock<MemoryId>,
    version: RwLock<u64>,
}

#[async_trait::async_trait]
impl MemoryStore for TestStore {
    async fn save(&self, e: NewMemory) -> Result<SaveOutcome, StoreError> {
        for existing in self.memories.read().unwrap().values() {
            if existing.content == e.content {
                return Ok(SaveOutcome::Duplicate(existing.id));
            }
        }
        let mut next = self.next_id.write().unwrap();
        let id = *next;
        *next += 1;
        let entry = MemoryEntry {
            id,
            kind: e.kind,
            content: e.content,
            tags: e.tags,
            importance: e.importance,
            pinned: e.pinned,
            origin: e.origin,
            source_session: e.source_session,
            created_at: Utc::now(),
            expires_at: e.expires_at,
        };
        self.memories.write().unwrap().insert(id, entry);
        *self.version.write().unwrap() += 1;
        Ok(SaveOutcome::Inserted(id))
    }

    async fn profile(&self, budget: ProfileBudget) -> Result<MemoryProfile, StoreError> {
        let memories = self.memories.read().unwrap();
        let mut entries: Vec<MemoryEntry> = memories
            .values()
            .filter(|e| e.pinned || e.importance >= 2)
            .cloned()
            .collect();
        entries.sort_by_key(|e| {
            (
                std::cmp::Reverse(e.pinned),
                std::cmp::Reverse(e.importance),
                e.id,
            )
        });
        entries.truncate(budget.max_entries);
        Ok(MemoryProfile {
            entries,
            total_count: memories.len(),
        })
    }

    async fn search(&self, q: &MemoryQuery) -> Result<Vec<ScoredMemory>, StoreError> {
        let memories = self.memories.read().unwrap();
        let q_lower = q.query.to_lowercase();
        let mut results: Vec<ScoredMemory> = memories
            .values()
            .filter(|e| q.kind.is_none_or(|k| e.kind == k))
            .filter(|e| e.content.to_lowercase().contains(&q_lower))
            .map(|e| ScoredMemory {
                entry: e.clone(),
                score: 1.0,
            })
            .collect();
        results.truncate(q.limit);
        Ok(results)
    }

    async fn forget(&self, id: MemoryId) -> Result<bool, StoreError> {
        *self.version.write().unwrap() += 1;
        Ok(self.memories.write().unwrap().remove(&id).is_some())
    }

    async fn touch(&self, _ids: &[MemoryId]) -> Result<(), StoreError> {
        Ok(())
    }

    async fn version(&self) -> Result<u64, StoreError> {
        Ok(*self.version.read().unwrap())
    }
}

/// Build a minimal [`MemoryEntry`] fixture.
fn entry(kind: MemoryKind, content: &str) -> MemoryEntry {
    MemoryEntry {
        id: 0,
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

#[tokio::test]
async fn per_profile_budgeters_limit_recall() {
    let mut user = UserMemory::new(Arc::new(TestStore::default()));
    user.remember("alice", "memory one", MemoryCategory::General, Decimal::ONE)
        .await
        .expect("remember one");
    user.remember("alice", "memory two", MemoryCategory::General, Decimal::ONE)
        .await
        .expect("remember two");
    user.remember(
        "alice",
        "memory three",
        MemoryCategory::General,
        Decimal::ONE,
    )
    .await
    .expect("remember three");

    // "alice" gets a tight budget (1 entry); "bob" falls back to the default 32.
    user.set_profile_budget(
        "alice",
        ProfileBudget {
            max_tokens: 1024,
            max_entries: 1,
        },
    );

    let alice = user.recall_profile("alice").await.expect("recall alice");
    assert_eq!(
        alice.entries.len(),
        1,
        "alice budget must cap profile recall"
    );
    assert_eq!(alice.total_count, 3);

    let bob = user.recall_profile("bob").await.expect("recall bob");
    assert_eq!(
        bob.entries.len(),
        3,
        "default budget must recall all entries"
    );
    assert_eq!(bob.total_count, 3);
}

#[tokio::test]
async fn remember_search_recall_roundtrip() {
    let user = UserMemory::new(Arc::new(TestStore::default()));
    user.remember(
        "alice",
        "Trader avoids PSU banks",
        MemoryCategory::Preference,
        Decimal::new(9, 1),
    )
    .await
    .expect("remember");

    let hits = user.search("alice", "PSU", 5).await.expect("search");
    assert_eq!(hits.len(), 1);
    assert!(hits[0].entry.content.contains("PSU"));

    let profile = user.recall_profile("alice").await.expect("recall profile");
    assert!(
        profile.entries.iter().any(|e| e.content.contains("PSU")),
        "recalled profile must include the remembered fact"
    );
}

#[tokio::test]
async fn forget_removes() {
    let user = UserMemory::new(Arc::new(TestStore::default()));
    let id = user
        .remember(
            "alice",
            "temporary note",
            MemoryCategory::General,
            Decimal::new(5, 1),
        )
        .await
        .expect("remember");

    let removed = user.forget("alice", id).await.expect("forget");
    assert!(removed, "forget must report the entry was removed");

    let hits = user.search("alice", "temporary", 5).await.expect("search");
    assert!(hits.is_empty(), "search must be empty after forget");
}

#[tokio::test]
async fn profile_section_placement_is_cached_prefix() {
    let user = UserMemory::new(Arc::new(TestStore::default()));
    let profile = MemoryProfile {
        entries: vec![entry(MemoryKind::Preference, "avoids PSU banks")],
        total_count: 1,
    };

    let sec = user.profile_section(&profile);

    assert_eq!(sec.placement, Placement::CachedPrefix);
    assert!(sec.text.contains("Preference: avoids PSU banks"));
}

#[tokio::test]
async fn recall_section_placement_is_turn_context() {
    let user = UserMemory::new(Arc::new(TestStore::default()));
    let recalled = vec![ScoredMemory {
        entry: entry(MemoryKind::Fact, "prefers weekly options"),
        score: 0.9,
    }];

    let sec = user.recall_section(&recalled);

    assert_eq!(sec.placement, Placement::TurnContext);
    assert!(sec.text.contains("prefers weekly options"));
}

#[tokio::test]
async fn assemble_produces_stable_cached_prefix() {
    let user = UserMemory::new(Arc::new(TestStore::default()));
    let e = entry(MemoryKind::Preference, "avoids PSU banks");
    let profile = MemoryProfile {
        entries: vec![e.clone()],
        total_count: 1,
    };
    let recalled = vec![ScoredMemory {
        entry: e,
        score: 0.9,
    }];

    let profile_sec = user.profile_section(&profile);
    let recall_sec = user.recall_section(&recalled);

    let a1 = user.assemble("base", &[profile_sec.clone(), recall_sec.clone()]);
    let a2 = user.assemble("base", &[profile_sec, recall_sec]);

    assert!(
        a1.cached_prefix.contains("avoids PSU banks"),
        "cached prefix must include the memory profile text"
    );
    assert!(
        a1.cached_prefix.starts_with("base"),
        "base prompt must lead the cached prefix"
    );
    assert_eq!(
        a1.cached_prefix, a2.cached_prefix,
        "re-assembly with identical sections must be byte-identical (cache hit)"
    );
    assert!(
        a1.turn_context
            .iter()
            .any(|s| s.id == "turn_recalled_memory"),
        "turn context must contain the recall section"
    );
    assert_eq!(a1.turn_context, a2.turn_context);
}

/// The profile argument selects a budget, never a separate store.
#[tokio::test]
async fn profiles_share_one_store() {
    let user = UserMemory::new(Arc::new(TestStore::default()));
    user.remember(
        "alice",
        "shared across profiles",
        MemoryCategory::General,
        Decimal::ONE,
    )
    .await
    .expect("remember");

    let bob = user.recall_profile("bob").await.expect("recall bob");
    assert_eq!(
        bob.total_count, 1,
        "profiles must share the underlying store"
    );
}
