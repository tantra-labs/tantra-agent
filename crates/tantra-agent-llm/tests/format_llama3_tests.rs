//! Golden tests for the Llama 3 prompt/tool format.

use serde_json::json;
use tantra_agent_core::ToolSpec;
use tantra_agent_llm::candle_gguf::{CandleGgufProvider, ChatTemplate};
use tantra_agent_llm::format::{ParsedChunk, format_for};
use tantra_agent_llm::{ChatMessage, ChatRole, LlmToolCall};

fn tool() -> ToolSpec {
    ToolSpec {
        name: "market_quote".into(),
        description: "Get a quote".into(),
        parameters: json!({"type":"object"}),
    }
}

fn parse(pieces: &[&str]) -> (Vec<Vec<ParsedChunk>>, Vec<ParsedChunk>) {
    let mut p = format_for(ChatTemplate::Llama3).new_parser();
    let fed = pieces.iter().map(|x| p.feed(x)).collect();
    (fed, p.finish())
}

#[test]
fn render_with_tools_golden() {
    let f = format_for(ChatTemplate::Llama3);
    let out = f.render(&[ChatMessage::user("quote?")], Some("SYS"), &[tool()]);
    assert!(out.starts_with("<|start_header_id|>system<|end_header_id|>\n\nSYS\n\nYou have access to the following functions. To call a function, reply with ONLY a JSON object of the form {\"name\": <function name>, \"parameters\": <object of arguments>} and nothing else. Call a function only when you need live data or a computation; otherwise answer directly. Never invent prices or statistics — call a function to get them, or say you do not have the data.\n\n"));
    assert!(out.contains("{\"type\":\"function\",\"function\":{\"name\":\"market_quote\",\"description\":\"Get a quote\",\"parameters\":{\"type\":\"object\"}}}<|eot_id|>"));
    assert!(out.contains("<|start_header_id|>user<|end_header_id|>\n\nquote?<|eot_id|>"));
    assert!(out.ends_with("<|start_header_id|>assistant<|end_header_id|>\n\n"));
}

#[test]
fn render_tool_call_and_result_turns() {
    let f = format_for(ChatTemplate::Llama3);
    let msgs = vec![
        ChatMessage::user("q"),
        ChatMessage::assistant_with_tools(
            "",
            vec![LlmToolCall {
                id: "call_1".into(),
                name: "market_quote".into(),
                arguments: "{\"symbol\":\"NSE:RELIANCE-EQ\"}".into(),
            }],
        ),
        ChatMessage {
            role: ChatRole::Tool,
            content: "{\"ltp\":1}".into(),
            tool_calls: vec![],
            tool_call_id: Some("call_1".into()),
            parts: vec![],
        },
    ];
    let out = f.render(&msgs, Some("S"), &[tool()]);
    assert!(out.contains("<|start_header_id|>assistant<|end_header_id|>\n\n{\"name\":\"market_quote\",\"parameters\":{\"symbol\":\"NSE:RELIANCE-EQ\"}}<|eot_id|>"));
    assert!(out.contains("<|start_header_id|>ipython<|end_header_id|>\n\n{\"ltp\":1}<|eot_id|>"));
}

#[test]
fn parser_buffers_json_call() {
    let (fed, fin) = parse(&[
        "{\"na",
        "me\": \"market_quote\", \"param",
        "eters\": {\"symbol\": \"NSE:RELIANCE-EQ\"}}",
    ]);
    assert!(fed.iter().all(|c| c.is_empty()));
    assert_eq!(
        fin,
        vec![ParsedChunk::ToolCall {
            name: "market_quote".into(),
            arguments: json!({"symbol":"NSE:RELIANCE-EQ"}),
            raw_arguments: "{\"symbol\":\"NSE:RELIANCE-EQ\"}".into(),
            is_malformed: false,
        }]
    );
}

#[test]
fn parser_python_tag_and_multiple_calls() {
    let (_, fin) = parse(&[
        "<|python_tag|>",
        "{\"name\": \"a\", \"parameters\": {}}; {\"name\": \"b\", \"arguments\": {\"x\": 1}}",
    ]);
    assert_eq!(fin.len(), 2);
    assert!(matches!(&fin[0], ParsedChunk::ToolCall { name, .. } if name == "a"));
    assert!(
        matches!(&fin[1], ParsedChunk::ToolCall { name, raw_arguments, .. } if name == "b" && raw_arguments == "{\"x\":1}")
    );
}

#[test]
fn parser_streams_plain_text() {
    let (fed, fin) = parse(&["NIFTY closed", " higher."]);
    assert_eq!(fed[0], vec![ParsedChunk::Text("NIFTY closed".into())]);
    assert_eq!(fed[1], vec![ParsedChunk::Text(" higher.".into())]);
    assert!(fin.is_empty());
}

#[test]
fn parser_malformed_becomes_text() {
    let (fed, fin) = parse(&["{not ", "json"]);
    assert!(fed.iter().all(|c| c.is_empty()));
    assert_eq!(fin, vec![ParsedChunk::Text("{not json".into())]);
}

#[test]
fn no_tools_matches_provider() {
    let msgs = vec![ChatMessage::user("hi"), ChatMessage::assistant("yo")];
    assert_eq!(
        format_for(ChatTemplate::Llama3).render(&msgs, Some("s"), &[]),
        CandleGgufProvider::format_prompt_for(ChatTemplate::Llama3, &msgs, Some("s"))
    );
}
