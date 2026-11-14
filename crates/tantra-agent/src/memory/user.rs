//! Persistent [`UserMemory`] layer with per-profile recall budgeters.
//!
//! A [`UserMemory`] wraps a shared [`MemoryEngine`] (and its [`MemoryStore`])
//! while keeping a per-profile [`ProfileBudget`] map. The store itself is shared
//! across profiles; the profile argument only selects which budget is used when
//! recalling the stable memory profile for the cached prompt prefix.
//!
//! Recall renderers delegate to `tantra-agent-prompt` so that cached-prefix
//! sections stay byte-stable across turns (KV prompt-cache hit) while per-turn
//! recalled memories land in the turn context.

use std::collections::HashMap;
use std::sync::Arc;

use rust_decimal::Decimal;

use tantra_agent_core::memory::{MemoryCategory, MemoryEngine};
use tantra_agent_core::prompt::default_assemble;
use tantra_agent_core::{
    MemoryId, MemoryProfile, MemoryStore, ProfileBudget, PromptAssembly, PromptSection,
    ScoredMemory, StoreError,
};
use tantra_agent_prompt::{render_memory_profile_section, render_turn_recalled_memory};

/// Persistent user memory layer with per-profile recall budgeters.
///
/// Zero-I/O: all storage goes through the injected [`MemoryStore`] port, so this
/// type is WASM-friendly and usable identically in backtesting and live trading.
#[derive(Clone)]
pub struct UserMemory {
    engine: MemoryEngine,
    budgets: HashMap<String, ProfileBudget>,
}

impl UserMemory {
    /// Wrap a shared [`MemoryStore`] with no per-profile budgets configured;
    /// every profile falls back to [`ProfileBudget::default()`].
    pub fn new(store: Arc<dyn MemoryStore>) -> Self {
        Self {
            engine: MemoryEngine::new(store),
            budgets: HashMap::new(),
        }
    }

    /// Wrap a shared [`MemoryStore`] with an explicit per-profile budget map.
    pub fn with_budgets(
        store: Arc<dyn MemoryStore>,
        budgets: HashMap<String, ProfileBudget>,
    ) -> Self {
        Self {
            engine: MemoryEngine::new(store),
            budgets,
        }
    }

    /// Set (or override) the recall budget for a profile.
    pub fn set_profile_budget(&mut self, profile: impl Into<String>, budget: ProfileBudget) {
        self.budgets.insert(profile.into(), budget);
    }

    /// The recall budget selected for a profile, falling back to
    /// [`ProfileBudget::default()`] when the profile has no explicit budget.
    pub fn profile_budget(&self, profile: &str) -> ProfileBudget {
        self.budgets.get(profile).copied().unwrap_or_default()
    }

    /// Access the underlying memory engine.
    pub fn engine(&self) -> &MemoryEngine {
        &self.engine
    }

    /// Remember a durable fact for a profile (stored in the shared store).
    ///
    /// The profile argument keeps the API symmetric; storage is shared.
    pub async fn remember(
        &self,
        _profile: &str,
        content: impl Into<String>,
        category: MemoryCategory,
        confidence: Decimal,
    ) -> Result<MemoryId, StoreError> {
        self.engine.remember(content, category, confidence).await
    }

    /// Search memory content relevant to a profile.
    pub async fn search(
        &self,
        _profile: &str,
        query: impl Into<String>,
        limit: usize,
    ) -> Result<Vec<ScoredMemory>, StoreError> {
        self.engine.search(query, limit).await
    }

    /// Recall the stable memory profile for a profile under its per-profile
    /// budget (falling back to [`ProfileBudget::default()`]).
    pub async fn recall_profile(&self, profile: &str) -> Result<MemoryProfile, StoreError> {
        self.engine.profile(self.profile_budget(profile)).await
    }

    /// Forget a memory by id.
    pub async fn forget(&self, _profile: &str, id: MemoryId) -> Result<bool, StoreError> {
        self.engine.forget(id).await
    }

    /// Render the stable memory profile as a cached-prefix [`PromptSection`].
    pub fn profile_section(&self, profile: &MemoryProfile) -> PromptSection {
        render_memory_profile_section(&profile.entries, profile.total_count)
    }

    /// Render relevance-recalled memories as a turn-context [`PromptSection`].
    pub fn recall_section(&self, recalled: &[ScoredMemory]) -> PromptSection {
        render_turn_recalled_memory(recalled)
    }

    /// Assemble a base prompt plus extra sections into a placement-typed
    /// assembly with a cache-stable prefix.
    pub fn assemble(&self, base: &str, extra_sections: &[PromptSection]) -> PromptAssembly {
        default_assemble(base, extra_sections)
    }
}

impl std::fmt::Debug for UserMemory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UserMemory").finish()
    }
}
