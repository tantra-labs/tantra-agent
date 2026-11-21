//! `present_view` chat tool (pure, `ToolRisk::ReadOnly`): lets the model choose what the
//! client's right-hand pane shows. See agent plan 0020.

use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tantra_agent_chat::permission::{ToolRisk, ToolTraits};
use tantra_agent_chat::tool::{ChatTool, PlanContext, ToolContext, ToolPlan};
use tantra_agent_core::{PresentViewRequest, ToolError, ToolOutput, ToolSpec, present_view};

/// System-prompt fragment hosts append when registering `present_view`.
pub const PRESENT_VIEW_GUIDANCE: &str = "\
Presenting results: use the data tools first to gather facts.
Then call `present_view` exactly once, as the final tool, to lay out the right-hand pane.
Choose the kind that fits the question: options questions -> options; strategy or backtest ->
backtest or strategy_lab; holdings -> portfolio; A-vs-B -> compare; otherwise analysis or report.
List the tools whose results the pane should show in `include_tools`; never retype their data.
Only put numbers you obtained from tools in the summary or highlights.
Keep the chat reply short because the pane carries the detail.";

fn spec_for<T: schemars::JsonSchema>(name: &str, description: String) -> ToolSpec {
    ToolSpec::new(
        name,
        description,
        serde_json::to_value(schemars::schema_for!(T)).unwrap_or_default(),
    )
}

fn parse_args<T: DeserializeOwned>(tool: &str, input: Value) -> Result<T, ToolError> {
    serde_json::from_value(input).map_err(|e| ToolError::InvalidArguments {
        tool: tool.to_string(),
        message: format!("invalid {tool} input: {e}"),
    })
}

/// `present_view` (pure).
pub struct PresentViewTool {
    spec: ToolSpec,
}

impl PresentViewTool {
    /// Construct the tool.
    pub fn new() -> Self {
        let spec = spec_for::<PresentViewRequest>(
            "present_view",
            "Open a view in the user's right-hand pane once you have gathered data. Choose kind (analysis|options|backtest|strategy_lab|portfolio|compare|report), a title, focus symbols, a short markdown summary, up to 12 highlight metrics, and include_tools = names of tools whose results this view should show. Do not restate numbers you did not get from a tool. Call at most once per answer, as the last tool.".to_string(),
        );
        Self { spec }
    }
}

impl Default for PresentViewTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ChatTool for PresentViewTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }
    fn traits(&self) -> ToolTraits {
        ToolTraits::new(ToolRisk::ReadOnly)
    }
    fn plan(&self, input: &Value, _ctx: &PlanContext<'_>) -> Result<ToolPlan, ToolError> {
        let kind = input
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or("analysis");
        let title = input.get("title").and_then(Value::as_str).unwrap_or("view");
        Ok(ToolPlan::new(format!("Present {kind} view '{title}'")))
    }
    async fn execute(&self, input: Value, _ctx: ToolContext) -> Result<ToolOutput, ToolError> {
        const NAME: &str = "present_view";
        let req: PresentViewRequest = parse_args(NAME, input)?;
        let env = present_view(&req).map_err(|e| ToolError::InvalidArguments {
            tool: NAME.to_string(),
            message: e.to_string(),
        })?;
        let payload = serde_json::to_value(env)
            .map_err(|e| ToolError::Execution(NAME.to_string(), e.to_string()))?;
        Ok(ToolOutput::ok(NAME, payload))
    }
}
