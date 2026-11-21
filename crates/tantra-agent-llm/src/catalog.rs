//! Model catalog port: the host owns provider construction; clients pick by id.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::LlmProvider;

/// Public description of a selectable model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Catalog id (e.g. `provider/model`).
    pub id: String,
    /// Display name.
    pub name: String,
    /// Whether the model supports tool calling.
    #[serde(default)]
    pub tool_call: bool,
    /// Context window in tokens, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<u32>,
}

/// Catalog lookup errors.
#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    /// No model with the given id.
    #[error("unknown model: {0}")]
    UnknownModel(String),
}

/// Port: list models and construct providers by id.
pub trait ModelCatalog: Send + Sync {
    /// All selectable models.
    fn models(&self) -> Vec<ModelInfo>;
    /// Id of the default model (empty if the catalog is empty).
    fn default_model(&self) -> String;
    /// Provider for the given model id.
    fn provider(&self, id: &str) -> Result<Arc<dyn LlmProvider>, CatalogError>;
}

/// Ordered static catalog; the first entry is the default unless overridden.
#[derive(Default)]
pub struct StaticCatalog {
    entries: Vec<(ModelInfo, Arc<dyn LlmProvider>)>,
    default: Option<String>,
}

impl StaticCatalog {
    /// Empty catalog.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a model with default capability metadata.
    pub fn with_model(
        self,
        id: impl Into<String>,
        name: impl Into<String>,
        provider: Arc<dyn LlmProvider>,
    ) -> Self {
        self.with_model_info(
            ModelInfo {
                id: id.into(),
                name: name.into(),
                tool_call: false,
                context: None,
            },
            provider,
        )
    }

    /// Add a model with full metadata.
    pub fn with_model_info(mut self, info: ModelInfo, provider: Arc<dyn LlmProvider>) -> Self {
        self.entries.push((info, provider));
        self
    }

    /// Override the default model id.
    pub fn with_default(mut self, id: impl Into<String>) -> Self {
        self.default = Some(id.into());
        self
    }
}

impl ModelCatalog for StaticCatalog {
    fn models(&self) -> Vec<ModelInfo> {
        self.entries.iter().map(|(m, _)| m.clone()).collect()
    }

    fn default_model(&self) -> String {
        if let Some(d) = &self.default {
            return d.clone();
        }
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
