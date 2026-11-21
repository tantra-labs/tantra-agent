//! The GGUF provider must speak each model's own chat template, and the tokenizer
//! must turn the template's control markers into single special tokens. Before this,
//! every model got ChatML text with unparsed markers (Llama-3.2 has no `<|im_start|>`
//! token at all), so replies degenerated into loops / "I am not sure how…".

use std::path::Path;

use tantra_agent_llm::ChatMessage;
use tantra_agent_llm::candle_gguf::{CandleGgufProvider, ChatTemplate, encode_prompt};

const LLAMA3_TEMPLATE_HEAD: &str = "{{- bos_token }}\n{%- for message in messages %}<|start_header_id|>{{ message['role'] }}<|end_header_id|>\n\n{{ message['content'] }}<|eot_id|>";
const CHATML_TEMPLATE_HEAD: &str = "{% for message in messages %}<|im_start|>{{ message['role'] }}\n{{ message['content'] }}<|im_end|>\n";

#[test]
fn template_is_detected_from_gguf_chat_template() {
    assert_eq!(
        ChatTemplate::detect(Some(LLAMA3_TEMPLATE_HEAD)),
        ChatTemplate::Llama3
    );
    assert_eq!(
        ChatTemplate::detect(Some(CHATML_TEMPLATE_HEAD)),
        ChatTemplate::ChatMl
    );
    assert_eq!(ChatTemplate::detect(None), ChatTemplate::ChatMl);
}

#[test]
fn llama3_prompt_uses_header_and_eot_markers() {
    let messages = vec![
        ChatMessage::user("Analyze NSE:NIFTY50-INDEX 1d"),
        ChatMessage::assistant("NIFTY 50 closed higher."),
        ChatMessage::user("And the trend?"),
    ];
    let p =
        CandleGgufProvider::format_prompt_for(ChatTemplate::Llama3, &messages, Some("Be terse."));
    assert!(
        p.starts_with("<|start_header_id|>system<|end_header_id|>\n\nBe terse.<|eot_id|>"),
        "{p}"
    );
    assert!(p.contains(
        "<|start_header_id|>user<|end_header_id|>\n\nAnalyze NSE:NIFTY50-INDEX 1d<|eot_id|>"
    ));
    assert!(p.contains(
        "<|start_header_id|>assistant<|end_header_id|>\n\nNIFTY 50 closed higher.<|eot_id|>"
    ));
    assert!(p.ends_with("<|start_header_id|>assistant<|end_header_id|>\n\n"));
    assert!(!p.contains("<|im_start|>"));
    // BOS is added by the tokenizer, never written into the text (no double BOS)
    assert!(!p.contains("<|begin_of_text|>"));
}

#[test]
fn chatml_prompt_is_unchanged_for_chatml_models() {
    let messages = vec![ChatMessage::user("hi")];
    assert_eq!(
        CandleGgufProvider::format_prompt_for(ChatTemplate::ChatMl, &messages, Some("sys")),
        CandleGgufProvider::format_prompt(&messages, Some("sys"))
    );
}

/// Real-tokenizer check against the workspace model, when it is present locally.
#[test]
fn llama3_markers_encode_as_single_special_tokens() {
    let candidates = [
        "../../../models/Llama-3.2-3B-Instruct-Q4_K_M.gguf",
        "../../models/Llama-3.2-3B-Instruct-Q4_K_M.gguf",
    ];
    let Some(path) = candidates.iter().map(Path::new).find(|p| p.exists()) else {
        eprintln!("skipping: Llama-3.2 GGUF not present");
        return;
    };
    let tok = shimmytok::Tokenizer::from_gguf_file(path).expect("tokenizer");
    assert_eq!(
        ChatTemplate::detect(tok.chat_template()),
        ChatTemplate::Llama3
    );
    let prompt = CandleGgufProvider::format_prompt_for(
        ChatTemplate::Llama3,
        &[ChatMessage::user("Analyze NSE:NIFTY50-INDEX 1d")],
        Some("sys"),
    );
    let ids = encode_prompt(&tok, ChatTemplate::Llama3, &prompt).expect("encode");
    let bos = tok.bos_token();
    assert_eq!(
        ids.iter().filter(|&&t| t == bos).count(),
        1,
        "exactly one BOS: {ids:?}"
    );
    assert_eq!(ids[0], bos);
    // <|start_header_id|> = 128006, <|end_header_id|> = 128007, <|eot_id|> = 128009
    assert_eq!(ids.iter().filter(|&&t| t == 128006).count(), 3, "{ids:?}");
    assert_eq!(ids.iter().filter(|&&t| t == 128009).count(), 2, "{ids:?}");
}
