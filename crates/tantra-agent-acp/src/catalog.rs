//! Model catalog port: the host owns provider construction; clients pick by id.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tantra_agent_llm::LlmProvider;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
}

#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    #[error("unknown model: {0}")]
    UnknownModel(String),
}

pub trait ModelCatalog: Send + Sync {
    fn models(&self) -> Vec<ModelInfo>;
    fn default_model(&self) -> String;
    fn provider(&self, id: &str) -> Result<Arc<dyn LlmProvider>, CatalogError>;
}

/// Ordered static catalog; the first entry is the default.
#[derive(Default)]
pub struct StaticCatalog {
    entries: Vec<(ModelInfo, Arc<dyn LlmProvider>)>,
}

impl StaticCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_model(
        mut self,
        id: impl Into<String>,
        name: impl Into<String>,
        provider: Arc<dyn LlmProvider>,
    ) -> Self {
        self.entries.push((
            ModelInfo {
                id: id.into(),
                name: name.into(),
            },
            provider,
        ));
        self
    }
}

impl ModelCatalog for StaticCatalog {
    fn models(&self) -> Vec<ModelInfo> {
        self.entries.iter().map(|(m, _)| m.clone()).collect()
    }

    fn default_model(&self) -> String {
        self.entries
            .first()
            .map(|(m, _)| m.id.clone())
            .unwrap_or_default()
    }

    fn provider(&self, id: &str) -> Result<Arc<dyn LlmProvider>, CatalogError> {
        self.entries
            .iter()
            .find(|(m, _)| m.id == id)
            .map(|(_, p)| Arc::clone(p))
            .ok_or_else(|| CatalogError::UnknownModel(id.to_string()))
    }
}
