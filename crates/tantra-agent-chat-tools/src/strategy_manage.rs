//! Strategy-manifest CRUD chat tool (plan 0007 Track 5 Phase 13, Jesse-MCP pattern).

use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;
use tantra_agent_chat::permission::{ToolRisk, ToolTraits};
use tantra_agent_chat::tool::{ChatTool, PlanContext, ToolContext, ToolPlan};
use tantra_agent_core::{ToolError, ToolOutput, ToolSpec};

use crate::ports::strategy_store::{StrategyParameterSpec, StrategySpec, StrategyStore};

/// Input arguments for `strategy_manage`.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StrategyManageInput {
    /// Operation: "create" | "list" | "inspect" | "delete".
    pub operation: String,
    /// Strategy id (required for create/inspect/delete).
    pub strategy_id: Option<String>,
    /// Human-readable name (create).
    pub name: Option<String>,
    /// Comma-separated instrument universe (create), e.g. "NSE:NIFTY50-INDEX".
    pub instruments: Option<String>,
    /// Parameter list (create): each element is {name, default, min, max, step}.
    pub parameters: Option<Vec<ParameterInput>>,
}

/// One declarative parameter supplied at strategy creation.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ParameterInput {
    /// Parameter name.
    pub name: String,
    /// Default value (decimal string).
    pub default: String,
    /// Inclusive minimum.
    pub min: String,
    /// Inclusive maximum.
    pub max: String,
    /// Step alignment.
    pub step: String,
}

/// Chat tool managing strategy manifests (create/list/inspect/delete) in the
/// strategy store (plan 0007 Track 5 Phase 13).
///
/// `ToolRisk::LocalWrite` — like the alert CRUD, it sits outside the
/// all-ReadOnly catalogue-count bundle.
pub struct StrategyManageTool {
    store: Arc<dyn StrategyStore>,
    spec: ToolSpec,
}

impl StrategyManageTool {
    /// Create new strategy-manage tool.
    pub fn new(store: Arc<dyn StrategyStore>) -> Self {
        let schema = schemars::schema_for!(StrategyManageInput);
        let spec = ToolSpec::new(
            "strategy_manage",
            "Create, list, inspect or delete declarative strategy manifests in the strategy store.",
            serde_json::to_value(schema).unwrap_or_default(),
        );
        Self { store, spec }
    }
}

#[async_trait]
impl ChatTool for StrategyManageTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn traits(&self) -> ToolTraits {
        ToolTraits::new(ToolRisk::LocalWrite)
    }

    fn plan(&self, input: &Value, _ctx: &PlanContext<'_>) -> Result<ToolPlan, ToolError> {
        let operation = input
            .get("operation")
            .and_then(Value::as_str)
            .unwrap_or("?");
        Ok(ToolPlan::new(format!("Strategy store `{operation}`")))
    }

    async fn execute(&self, input: Value, _ctx: ToolContext) -> Result<ToolOutput, ToolError> {
        let args: StrategyManageInput =
            serde_json::from_value(input).map_err(|e| ToolError::InvalidArguments {
                tool: "strategy_manage".to_string(),
                message: format!("schema validation failure: {e}"),
            })?;

        match args.operation.as_str() {
            "create" => {
                let spec = build_spec(&args)?;
                let stored = self
                    .store
                    .save_strategy(&spec)
                    .await
                    .map_err(|e| e.into_tool_error("strategy_manage"))?;
                Ok(ToolOutput::ok(
                    "strategy_manage",
                    serde_json::json!({ "strategy": spec_json(&stored) }),
                ))
            }
            "list" => {
                let specs = self
                    .store
                    .list_strategies()
                    .await
                    .map_err(|e| e.into_tool_error("strategy_manage"))?;
                let list: Vec<Value> = specs.iter().map(spec_json).collect();
                Ok(ToolOutput::ok(
                    "strategy_manage",
                    serde_json::json!({ "strategies": list, "count": list.len() }),
                ))
            }
            "inspect" => {
                let id = args
                    .strategy_id
                    .ok_or_else(|| ToolError::InvalidArguments {
                        tool: "strategy_manage".to_string(),
                        message: "inspect requires a strategy_id".to_string(),
                    })?;
                let spec = self
                    .store
                    .get_strategy(&id)
                    .await
                    .map_err(|e| e.into_tool_error("strategy_manage"))?;
                Ok(ToolOutput::ok(
                    "strategy_manage",
                    serde_json::json!({ "strategy": spec_json(&spec) }),
                ))
            }
            "delete" => {
                let id = args
                    .strategy_id
                    .ok_or_else(|| ToolError::InvalidArguments {
                        tool: "strategy_manage".to_string(),
                        message: "delete requires a strategy_id".to_string(),
                    })?;
                let deleted = self
                    .store
                    .delete_strategy(&id)
                    .await
                    .map_err(|e| e.into_tool_error("strategy_manage"))?;
                Ok(ToolOutput::ok(
                    "strategy_manage",
                    serde_json::json!({ "strategy_id": id, "deleted": deleted }),
                ))
            }
            other => Err(ToolError::InvalidArguments {
                tool: "strategy_manage".to_string(),
                message: format!("operation must be create|list|inspect|delete, got `{other}`"),
            }),
        }
    }
}

/// Builds a [`StrategySpec`] from the create-operation input.
pub(crate) fn build_spec(args: &StrategyManageInput) -> Result<StrategySpec, ToolError> {
    let id = args
        .strategy_id
        .clone()
        .ok_or_else(|| ToolError::InvalidArguments {
            tool: "strategy_manage".to_string(),
            message: "create requires a strategy_id".to_string(),
        })?;
    if id.trim().is_empty() {
        return Err(ToolError::InvalidArguments {
            tool: "strategy_manage".to_string(),
            message: "strategy_id must not be empty".to_string(),
        });
    }
    let instruments: Vec<String> = args
        .instruments
        .clone()
        .map(|csv| {
            csv.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();
    if instruments.is_empty() {
        return Err(ToolError::InvalidArguments {
            tool: "strategy_manage".to_string(),
            message: "create requires a non-empty instruments universe".to_string(),
        });
    }
    let parameters = args
        .parameters
        .clone()
        .unwrap_or_default()
        .into_iter()
        .map(|p| StrategyParameterSpec {
            name: p.name,
            default: p.default,
            min: p.min,
            max: p.max,
            step: p.step,
        })
        .collect();
    let now = Utc::now();
    Ok(StrategySpec {
        strategy_id: id,
        name: args.name.clone().unwrap_or_default(),
        instruments,
        parameters,
        created_at: now,
        updated_at: now,
    })
}

/// Renders one strategy spec as JSON for the tool output.
pub(crate) fn spec_json(spec: &StrategySpec) -> Value {
    serde_json::json!({
        "strategy_id": spec.strategy_id,
        "name": spec.name,
        "instruments": spec.instruments,
        "parameters": spec.parameters,
        "created_at": spec.created_at,
        "updated_at": spec.updated_at,
    })
}
