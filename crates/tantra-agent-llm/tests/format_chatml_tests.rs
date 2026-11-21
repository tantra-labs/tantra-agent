//! Golden tests for the ChatML/Hermes prompt/tool format.

use serde_json::json;
use tantra_agent_core::ToolSpec;
use tantra_agent_llm::candle_gguf::{CandleGgufProvider, ChatTemplate};
use tantra_agent_llm::format::{ParsedChunk, format_for, parse_override};
use tantra_agent_llm::{ChatMessage, ChatRole};

fn tool() -> ToolSpec {
    ToolSpec {
        name: "market_quote".into(),
        description: "Get a quote".into(),
        parameters: json!({"type":"object"}),
    }
}

fn text_of(chunks: &[ParsedChunk]) -> String {
    chunks
        .iter()
        .filter_map(|c| match c {
            ParsedChunk::Text(t) => Some(t.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn render_with_tools_golden() {
    let out =
        format_for(ChatTemplate::ChatMl).render(&[ChatMessage::user("q")], Some("SYS"), &[tool()]);
    assert!(
        out.starts_with("<|im_start|>system\nSYS\n\n# Tools\n\nYou may call one or more functions")
    );
    assert!(out.contains("<tools>\n{\"type\":\"function\",\"function\":{\"name\":\"market_quote\",\"description\":\"Get a quote\",\"parameters\":{\"type\":\"object\"}}}\n</tools>"));
    assert!(out.contains("<tool_call>\n{\"name\": <function-name>, \"arguments\": <args-json-object>}\n</tool_call><|im_end|>\n"));
    assert!(out.ends_with("<|im_start|>assistant\n"));
}

#[test]
fn no_tools_identical_to_provider() {
    let msgs = vec![ChatMessage::user("hi"), ChatMessage::assistant("yo")];
    assert_eq!(
        format_for(ChatTemplate::ChatMl).render(&msgs, Some("sys"), &[]),
        CandleGgufProvider::format_prompt(&msgs, Some("sys"))
    );
}

#[test]
fn tool_result_rendered_as_tool_response() {
    let msgs = vec![ChatMessage {
        role: ChatRole::Tool,
        content: "{\"ltp\":1}".into(),
        tool_calls: vec![],
        tool_call_id: Some("call_1".into()),
        parts: vec![],
    }];
    let out = format_for(ChatTemplate::ChatMl).render(&msgs, Some("s"), &[tool()]);
    assert!(
        out.contains(
            "<|im_start|>user\n<tool_response>\n{\"ltp\":1}\n</tool_response><|im_end|>\n"
        )
    );
}

#[test]
fn parser_text_then_call_across_pieces() {
    let mut p = format_for(ChatTemplate::ChatMl).new_parser();
    let mut all = p.feed("Let me check. <tool_");
    assert_eq!(text_of(&all), "Let me check. ");
    all.extend(p.feed("call>\n{\"name\": \"market_quote\", \"arguments\": {\"symbol\": \"NSE:TCS\"}}\n</tool_call>"));
    all.extend(p.finish());
    let calls: Vec<_> = all
        .iter()
        .filter(|c| matches!(c, ParsedChunk::ToolCall { .. }))
        .collect();
    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0],
        &ParsedChunk::ToolCall {
            name: "market_quote".into(),
            arguments: json!({"symbol":"NSE:TCS"}),
            raw_arguments: "{\"symbol\":\"NSE:TCS\"}".into(),
            is_malformed: false,
        }
    );
    assert_eq!(text_of(&all), "Let me check. ");
}

#[test]
fn parser_holds_partial_tag_then_releases() {
    let mut p = format_for(ChatTemplate::ChatMl).new_parser();
    let first = p.feed("up <");
    assert_eq!(text_of(&first), "up ");
    let mut rest = p.feed("5 points");
    rest.extend(p.finish());
    assert_eq!(text_of(&rest), "<5 points");
}

#[test]
fn parser_unparsable_or_unterminated_is_text() {
    let mut p = format_for(ChatTemplate::ChatMl).new_parser();
    let mut all = p.feed("<tool_call>nonsense</tool_call>");
    all.extend(p.finish());
    assert_eq!(all, vec![ParsedChunk::Text("nonsense".into())]);

    let mut p = format_for(ChatTemplate::ChatMl).new_parser();
    let mut all = p.feed("<tool_call>{oops");
    all.extend(p.finish());
    assert_eq!(all, vec![ParsedChunk::Text("{oops".into())]);
}

#[test]
fn override_parsing() {
    assert_eq!(parse_override(Some("llama3")), Some(ChatTemplate::Llama3));
    assert_eq!(parse_override(Some("LLAMA3")), Some(ChatTemplate::Llama3));
    assert_eq!(parse_override(Some("chatml")), Some(ChatTemplate::ChatMl));
    assert_eq!(parse_override(Some("hermes")), Some(ChatTemplate::ChatMl));
    assert_eq!(parse_override(Some("qwen")), Some(ChatTemplate::ChatMl));
    assert_eq!(parse_override(Some("garbage")), None);
    assert_eq!(parse_override(None), None);
}
