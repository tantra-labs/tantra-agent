//! Per-model-family prompt and tool-calling format adapters for local GGUF models.
//!
//! A [`ModelFormat`] lays a conversation (plus the tool list) out as a prompt string and
//! hands back an [`OutputParser`] that splits the model's raw output into plain text and
//! tool calls. This module is pure: no I/O, no clock.

use serde_json::Value;
use tantra_agent_core::ToolSpec;

use crate::ChatMessage;
use crate::candle_gguf::ChatTemplate;

pub mod chatml;
pub mod llama3;

/// One model family's wire format: how a conversation + tool list is laid out as a
/// prompt, and how the model's raw output is split into text and tool calls.
pub trait ModelFormat: Send + Sync {
    /// The chat template this format implements.
    fn template(&self) -> ChatTemplate;
    /// Render the full prompt, ending with the open assistant header.
    fn render(
        &self,
        messages: &[ChatMessage],
        system_prompt: Option<&str>,
        tools: &[ToolSpec],
    ) -> String;
    /// A fresh streaming parser for one generation.
    fn new_parser(&self) -> Box<dyn OutputParser>;
}

/// Streaming output parser: fed decoded pieces in order; returns what can be emitted now.
pub trait OutputParser: Send {
    /// Feed one decoded piece.
    fn feed(&mut self, piece: &str) -> Vec<ParsedChunk>;
    /// End of generation: flush buffered text / parse a buffered call.
    fn finish(&mut self) -> Vec<ParsedChunk>;
}

/// A unit of parsed model output.
#[derive(Debug, Clone, PartialEq)]
pub enum ParsedChunk {
    /// Plain assistant text.
    Text(String),
    /// A parsed tool call.
    ToolCall {
        /// Function name.
        name: String,
        /// Parsed arguments.
        arguments: Value,
        /// Compact JSON of the arguments.
        raw_arguments: String,
        /// Whether the arguments failed to parse.
        is_malformed: bool,
    },
}

/// The format implementation for a chat template.
pub fn format_for(template: ChatTemplate) -> Box<dyn ModelFormat> {
    match template {
        ChatTemplate::Llama3 => Box::new(llama3::Llama3Format),
        ChatTemplate::ChatMl => Box::new(chatml::ChatMlFormat),
    }
}

/// Parse a `TANTRA_GGUF_FORMAT` override value (`llama3`, `chatml`, `hermes`, `qwen`).
pub fn parse_override(value: Option<&str>) -> Option<ChatTemplate> {
    match value?.trim().to_ascii_lowercase().as_str() {
        "llama3" => Some(ChatTemplate::Llama3),
        "chatml" | "hermes" | "qwen" => Some(ChatTemplate::ChatMl),
        _ => None,
    }
}

fn json_str(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".to_string())
}

fn json_val(v: &Value) -> String {
    serde_json::to_string(v).unwrap_or_else(|_| "null".to_string())
}

/// One compact `{"type":"function","function":{...}}` line per tool (stable key order).
pub(crate) fn tool_lines(tools: &[ToolSpec]) -> Vec<String> {
    tools
        .iter()
        .map(|t| {
            format!(
                "{{\"type\":\"function\",\"function\":{{\"name\":{},\"description\":{},\"parameters\":{}}}}}",
                json_str(&t.name),
                json_str(&t.description),
                json_val(&t.parameters)
            )
        })
        .collect()
}

/// Parse a raw JSON arguments string, falling back to the raw string as a JSON string.
pub(crate) fn args_value(raw: &str) -> Value {
    serde_json::from_str(raw).unwrap_or_else(|_| Value::String(raw.to_string()))
}

/// `{"name":<name>,"<key>":<args>}` compact JSON for rendering a prior call.
pub(crate) fn call_json(name: &str, key: &str, raw_arguments: &str) -> String {
    format!(
        "{{\"name\":{},\"{key}\":{}}}",
        json_str(name),
        json_val(&args_value(raw_arguments))
    )
}

/// Build a tool-call chunk from `{"name": .., "parameters"|"arguments": {..}}`.
pub(crate) fn call_from_value(v: &Value) -> Option<ParsedChunk> {
    let obj = v.as_object()?;
    let name = obj.get("name")?.as_str()?;
    if name.is_empty() {
        return None;
    }
    let args = obj.get("parameters").or_else(|| obj.get("arguments"))?;
    if !args.is_object() {
        return None;
    }
    Some(ParsedChunk::ToolCall {
        name: name.to_string(),
        raw_arguments: json_val(args),
        arguments: args.clone(),
        is_malformed: false,
    })
}
