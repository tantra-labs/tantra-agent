//! ChatML format with the Hermes/Qwen `<tool_call>` convention.

use tantra_agent_core::ToolSpec;

use super::{ModelFormat, OutputParser, ParsedChunk, call_from_value, call_json, tool_lines};
use crate::candle_gguf::{ChatTemplate, DEFAULT_SYSTEM_PROMPT};
use crate::{ChatMessage, ChatRole};

const OPEN: &str = "<tool_call>";
const CLOSE: &str = "</tool_call>";

/// ChatML prompt layout and `<tool_call>` parsing.
pub struct ChatMlFormat;

fn turn(prompt: &mut String, role: &str, content: &str) {
    prompt.push_str("<|im_start|>");
    prompt.push_str(role);
    prompt.push('\n');
    prompt.push_str(content);
    prompt.push_str("<|im_end|>\n");
}

impl ModelFormat for ChatMlFormat {
    fn template(&self) -> ChatTemplate {
        ChatTemplate::ChatMl
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
                "\n\n# Tools\n\nYou may call one or more functions to assist with the user query. Never invent prices or statistics — call a function to get them, or say you do not have the data.\n\nYou are provided with function signatures within <tools></tools> XML tags:\n<tools>\n",
            );
            sys.push_str(&tool_lines(tools).join("\n"));
            sys.push_str(
                "\n</tools>\n\nFor each function call, return a json object with function name and arguments within <tool_call></tool_call> XML tags:\n<tool_call>\n{\"name\": <function-name>, \"arguments\": <args-json-object>}\n</tool_call>",
            );
        }
        if !sys.trim().is_empty() {
            turn(&mut prompt, "system", sys.trim());
        }
        for msg in messages {
            match msg.role {
                ChatRole::System => turn(&mut prompt, "system", msg.content.trim()),
                ChatRole::User => turn(&mut prompt, "user", msg.content.trim()),
                ChatRole::Tool => turn(
                    &mut prompt,
                    "user",
                    &format!("<tool_response>\n{}\n</tool_response>", msg.content.trim()),
                ),
                ChatRole::Assistant => {
                    let mut body = msg.content.trim().to_string();
                    for c in &msg.tool_calls {
                        body.push_str("\n<tool_call>\n");
                        body.push_str(&call_json(&c.name, "arguments", &c.arguments));
                        body.push_str("\n</tool_call>");
                    }
                    turn(&mut prompt, "assistant", &body);
                }
            }
        }
        prompt.push_str("<|im_start|>assistant\n");
        prompt
    }

    fn new_parser(&self) -> Box<dyn OutputParser> {
        Box::new(ChatMlParser {
            buf: String::new(),
            in_call: false,
        })
    }
}

struct ChatMlParser {
    buf: String,
    in_call: bool,
}

fn call_or_text(inner: &str) -> ParsedChunk {
    serde_json::from_str::<serde_json::Value>(inner.trim())
        .ok()
        .and_then(|v| call_from_value(&v))
        .unwrap_or_else(|| ParsedChunk::Text(inner.to_string()))
}

impl OutputParser for ChatMlParser {
    fn feed(&mut self, piece: &str) -> Vec<ParsedChunk> {
        self.buf.push_str(piece);
        let mut out = Vec::new();
        loop {
            if self.in_call {
                let Some(i) = self.buf.find(CLOSE) else { break };
                let inner = self.buf[..i].to_string();
                self.buf.drain(..i + CLOSE.len());
                out.push(call_or_text(&inner));
                self.in_call = false;
            } else if let Some(i) = self.buf.find(OPEN) {
                if i > 0 {
                    out.push(ParsedChunk::Text(self.buf[..i].to_string()));
                }
                self.buf.drain(..i + OPEN.len());
                self.in_call = true;
            } else {
                let hold = (1..OPEN.len())
                    .rev()
                    .find(|&k| self.buf.ends_with(&OPEN[..k]))
                    .unwrap_or(0);
                let emit = self.buf.len() - hold;
                if emit > 0 {
                    out.push(ParsedChunk::Text(self.buf[..emit].to_string()));
                    self.buf.drain(..emit);
                }
                break;
            }
        }
        out
    }

    fn finish(&mut self) -> Vec<ParsedChunk> {
        let buf = std::mem::take(&mut self.buf);
        if buf.is_empty() {
            return Vec::new();
        }
        if self.in_call {
            self.in_call = false;
            vec![call_or_text(&buf)]
        } else {
            vec![ParsedChunk::Text(buf)]
        }
    }
}
