//! Llama 3.1/3.2 header format with JSON function calling.

use tantra_agent_core::ToolSpec;

use super::{ModelFormat, OutputParser, ParsedChunk, call_from_value, call_json, tool_lines};
use crate::candle_gguf::{ChatTemplate, DEFAULT_SYSTEM_PROMPT};
use crate::{ChatMessage, ChatRole};

const PYTHON_TAG: &str = "<|python_tag|>";

/// Llama 3.x prompt layout and `{"name", "parameters"}` call parsing.
pub struct Llama3Format;

fn turn(prompt: &mut String, role: &str, content: &str) {
    prompt.push_str("<|start_header_id|>");
    prompt.push_str(role);
    prompt.push_str("<|end_header_id|>\n\n");
    prompt.push_str(content.trim());
    prompt.push_str("<|eot_id|>");
}

impl ModelFormat for Llama3Format {
    fn template(&self) -> ChatTemplate {
        ChatTemplate::Llama3
    }

    fn render(
        &self,
        messages: &[ChatMessage],
        system_prompt: Option<&str>,
        tools: &[ToolSpec],
    ) -> String {
        let mut prompt = String::new();
        let mut sys = system_prompt.unwrap_or(DEFAULT_SYSTEM_PROMPT).to_string();
        if !tools.is_empty() {
            sys.push_str(
                "\n\nYou have access to the following functions. To call a function, reply with ONLY a JSON object of the form {\"name\": <function name>, \"parameters\": <object of arguments>} and nothing else. Call a function only when you need live data or a computation; otherwise answer directly. Never invent prices or statistics — call a function to get them, or say you do not have the data.\n\n",
            );
            sys.push_str(&tool_lines(tools).join("\n\n"));
        }
        if !sys.trim().is_empty() {
            turn(&mut prompt, "system", &sys);
        }
        for msg in messages {
            match msg.role {
                ChatRole::System => turn(&mut prompt, "system", &msg.content),
                ChatRole::User => turn(&mut prompt, "user", &msg.content),
                ChatRole::Tool => turn(&mut prompt, "ipython", &msg.content),
                ChatRole::Assistant => {
                    if msg.tool_calls.is_empty() {
                        turn(&mut prompt, "assistant", &msg.content);
                    } else {
                        let calls = msg
                            .tool_calls
                            .iter()
                            .map(|c| call_json(&c.name, "parameters", &c.arguments))
                            .collect::<Vec<_>>()
                            .join("; ");
                        let text = msg.content.trim();
                        let body = if text.is_empty() {
                            calls
                        } else {
                            format!("{text}\n{calls}")
                        };
                        turn(&mut prompt, "assistant", &body);
                    }
                }
            }
        }
        prompt.push_str("<|start_header_id|>assistant<|end_header_id|>\n\n");
        prompt
    }

    fn new_parser(&self) -> Box<dyn OutputParser> {
        Box::new(Llama3Parser {
            state: State::Undecided,
            buf: String::new(),
        })
    }
}

enum State {
    Undecided,
    Text,
    Buffering,
}

struct Llama3Parser {
    state: State,
    buf: String,
}

impl OutputParser for Llama3Parser {
    fn feed(&mut self, piece: &str) -> Vec<ParsedChunk> {
        match self.state {
            State::Text => vec![ParsedChunk::Text(piece.to_string())],
            State::Buffering => {
                self.buf.push_str(piece);
                Vec::new()
            }
            State::Undecided => {
                self.buf.push_str(piece);
                let trimmed = self.buf.trim_start();
                if trimmed.is_empty() {
                    return Vec::new();
                }
                if trimmed.starts_with('{') || trimmed.starts_with(PYTHON_TAG) {
                    self.state = State::Buffering;
                    return Vec::new();
                }
                if PYTHON_TAG.starts_with(trimmed) {
                    return Vec::new();
                }
                self.state = State::Text;
                vec![ParsedChunk::Text(std::mem::take(&mut self.buf))]
            }
        }
    }

    fn finish(&mut self) -> Vec<ParsedChunk> {
        let buf = std::mem::take(&mut self.buf);
        match self.state {
            State::Text => Vec::new(),
            _ if buf.is_empty() => Vec::new(),
            State::Undecided => vec![ParsedChunk::Text(buf)],
            State::Buffering => match parse_calls(&buf) {
                Some(calls) => calls,
                None => vec![ParsedChunk::Text(buf)],
            },
        }
    }
}

/// Parse one or more `;`-separated call objects; `None` unless at least one and all valid.
fn parse_calls(buf: &str) -> Option<Vec<ParsedChunk>> {
    let mut rest = buf.trim_start();
    rest = rest.strip_prefix(PYTHON_TAG).unwrap_or(rest);
    let mut calls = Vec::new();
    loop {
        rest = rest.trim_start_matches(|c: char| c == ';' || c.is_whitespace());
        if rest.is_empty() {
            break;
        }
        let mut de = serde_json::Deserializer::from_str(rest).into_iter::<serde_json::Value>();
        let value = de.next()?.ok()?;
        let used = de.byte_offset();
        calls.push(call_from_value(&value)?);
        rest = &rest[used..];
    }
    if calls.is_empty() { None } else { Some(calls) }
}
