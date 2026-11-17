//! Comprehensive test suite for tantra-agent-chat (§5, Phase A).

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use tokio::time::timeout;

use tantra_agent_chat::{
    ChatAgent, ChatConfig, ConfirmOverride, FakeChatTool, InMemorySessionStore, PermissionDecision,
    SessionEvent, SessionSelector, ToolRegistry, ToolRisk, TurnOutcome, run_turn_headless,
};
use tantra_agent_core::{RuntimeMode, SessionScope};
use tantra_agent_llm::ScriptedFakeLlm;

#[tokio::test]
async fn test_basic_conversation_turn() {
    let fake_llm = Arc::new(ScriptedFakeLlm::new("test-claude"));
    fake_llm.push_text_response("NIFTY 50 14-period RSI is currently 58.2.");

    let store = Arc::new(InMemorySessionStore::new());
    let registry = ToolRegistry::new();

    let agent = ChatAgent::builder(fake_llm, registry, store.clone())
        .build()
        .expect("agent build failed");

    let scope = SessionScope::new("default", RuntimeMode::Paper);
    let (handle, mut events) = agent
        .open(SessionSelector::New(scope))
        .await
        .expect("open failed");

    let _turn_id = handle
        .submit("What is the RSI of Nifty 50?")
        .await
        .expect("submit failed");

    let mut text_acc = String::new();
    let mut turn_completed = false;

    while let Ok(Some(event)) = timeout(Duration::from_secs(5), events.recv()).await {
        match event {
            SessionEvent::TextDelta { text, .. } => {
                text_acc.push_str(&text);
            }
            SessionEvent::TurnEnded { outcome, .. } => {
                assert_eq!(outcome, TurnOutcome::Completed);
                turn_completed = true;
                break;
            }
            _ => {}
        }
    }

    assert!(turn_completed);
    assert_eq!(text_acc, "NIFTY 50 14-period RSI is currently 58.2.");

    let history = handle.history().await.expect("history retrieval");
    assert_eq!(history.len(), 2); // 1 user + 1 assistant
    assert_eq!(history[0].role, "user");
    assert_eq!(history[1].role, "assistant");
    assert!(history[1].request_hash.is_some());
}

#[tokio::test]
async fn test_tool_execution_flow() {
    let fake_llm = Arc::new(ScriptedFakeLlm::new("test-claude"));

    // Round 1: Model requests tool call
    fake_llm.push_tool_call_response("call_101", "market_quote", json!({ "symbol": "RELIANCE" }));

    // Round 2: Model receives tool result and produces final answer
    fake_llm.push_text_response("Reliance is trading at 2843.15.");

    let store = Arc::new(InMemorySessionStore::new());
    let mut registry = ToolRegistry::new();

    let fake_tool = Arc::new(FakeChatTool::new("market_quote", ToolRisk::ReadOnly));
    fake_tool.set_result(Ok(json!({ "price": "2843.15", "volume": 1200000 })));
    registry.register(fake_tool);

    let agent = ChatAgent::builder(fake_llm, registry, store.clone())
        .build()
        .expect("agent build failed");

    let scope = SessionScope::new("default", RuntimeMode::Live);
    let (handle, mut events) = agent
        .open(SessionSelector::New(scope))
        .await
        .expect("open failed");

    let _turn_id = handle.submit("Quote Reliance").await.expect("submit");

    let mut tool_started = false;
    let mut tool_finished = false;
    let mut turn_ended = false;

    while let Ok(Some(event)) = timeout(Duration::from_secs(5), events.recv()).await {
        match event {
            SessionEvent::ToolStarted { name, eager, .. } => {
                assert_eq!(name, "market_quote");
                assert!(!eager);
                tool_started = true;
            }
            SessionEvent::ToolFinished { name, outcome, .. } => {
                assert_eq!(name, "market_quote");
                assert_eq!(outcome, tantra_agent_chat::ToolOutcome::Success);
                tool_finished = true;
            }
            SessionEvent::TurnEnded {
                outcome, rounds, ..
            } => {
                assert_eq!(outcome, TurnOutcome::Completed);
                assert_eq!(rounds, 2);
                turn_ended = true;
                break;
            }
            _ => {}
        }
    }

    assert!(tool_started);
    assert!(tool_finished);
    assert!(turn_ended);

    let history = handle.history().await.expect("history");
    assert_eq!(history.len(), 4); // user, assistant(call), tool(result), assistant(text)
    assert_eq!(history[0].role, "user");
    assert_eq!(history[1].role, "assistant");
    assert_eq!(history[2].role, "tool");
    assert_eq!(history[3].role, "assistant");
}

#[tokio::test]
async fn test_permission_prompt_denied_continues_loop() {
    let fake_llm = Arc::new(ScriptedFakeLlm::new("test-claude"));

    // Round 1: Model requests watchlist edit (elevated local write)
    fake_llm.push_tool_call_response(
        "call_202",
        "watchlist_edit",
        json!({ "symbol": "TCS", "action": "add" }),
    );

    // Round 2: Model observes denial and explains gracefully
    fake_llm.push_text_response("Understood, I will not modify your watchlist.");

    let store = Arc::new(InMemorySessionStore::new());
    let mut registry = ToolRegistry::new();

    let write_tool = Arc::new(
        FakeChatTool::new("watchlist_edit", ToolRisk::LocalWrite)
            .with_confirm(ConfirmOverride::Always),
    );
    registry.register(write_tool);

    let agent = ChatAgent::builder(fake_llm, registry, store.clone())
        .build()
        .expect("agent build");

    let scope = SessionScope::new("default", RuntimeMode::Paper);
    let (handle, mut events) = agent.open(SessionSelector::New(scope)).await.expect("open");

    let _turn_id = handle.submit("Add TCS to watchlist").await.expect("submit");

    let mut permission_prompted = false;
    let mut denied_event_seen = false;

    while let Ok(Some(event)) = timeout(Duration::from_secs(5), events.recv()).await {
        match event {
            SessionEvent::PermissionRequested(prompt) => {
                assert_eq!(prompt.tool, "watchlist_edit");
                permission_prompted = true;
                if let Some(responder) = prompt.responder {
                    responder
                        .respond(PermissionDecision::Deny {
                            feedback: Some("User cancelled operation".into()),
                        })
                        .expect("respond");
                }
            }
            SessionEvent::ToolFinished {
                outcome: tantra_agent_chat::ToolOutcome::Denied(ref msg),
                ..
            } => {
                assert!(msg.contains("User cancelled"));
                denied_event_seen = true;
            }
            SessionEvent::TurnEnded { outcome, .. } => {
                assert_eq!(outcome, TurnOutcome::Completed);
                break;
            }
            _ => {}
        }
    }

    assert!(permission_prompted);
    assert!(denied_event_seen);
}

#[tokio::test]
async fn test_round_limit_exceeded() {
    let fake_llm = Arc::new(ScriptedFakeLlm::new("test-claude"));

    // Infinite tool calls scripted
    for i in 0..10 {
        fake_llm.push_tool_call_response(format!("call_{i}"), "ping", json!({}));
    }

    let store = Arc::new(InMemorySessionStore::new());
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(FakeChatTool::new("ping", ToolRisk::ReadOnly)));

    let config = ChatConfig {
        max_rounds: 3,
        ..Default::default()
    };

    let agent = ChatAgent::builder(fake_llm, registry, store.clone())
        .config(config)
        .build()
        .expect("agent build");

    let (handle, mut events) = agent
        .open(SessionSelector::New(SessionScope::new(
            "test",
            RuntimeMode::Live,
        )))
        .await
        .expect("open");

    handle.submit("Keep pinging").await.expect("submit");

    let mut hit_limit = false;
    while let Ok(Some(event)) = timeout(Duration::from_secs(5), events.recv()).await {
        if let SessionEvent::TurnEnded { outcome, .. } = event {
            assert_eq!(outcome, TurnOutcome::RoundLimit { limit: 3 });
            hit_limit = true;
            break;
        }
    }
    assert!(hit_limit);
}

#[tokio::test]
async fn test_headless_run() {
    let fake_llm = Arc::new(ScriptedFakeLlm::new("test-claude"));
    fake_llm.push_text_response("Headless execution response.");

    let store = Arc::new(InMemorySessionStore::new());
    let registry = ToolRegistry::new();

    let agent = ChatAgent::builder(fake_llm, registry, store)
        .build()
        .expect("agent");

    let scope = SessionScope::new("cli", RuntimeMode::Backtest);
    let (outcome, events) = run_turn_headless(&agent, SessionSelector::New(scope), "Run headless")
        .await
        .expect("headless run");

    assert_eq!(outcome, TurnOutcome::Completed);
    assert!(!events.is_empty());
}

#[test]
fn test_tool_registry_tantra_prefix_resolution() {
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(FakeChatTool::new(
        "market_quote",
        ToolRisk::ReadOnly,
    )));
    registry.register(Arc::new(FakeChatTool::new(
        "research_calc",
        ToolRisk::ReadOnly,
    )));
    registry.register(Arc::new(FakeChatTool::new(
        "screener_scan",
        ToolRisk::ReadOnly,
    )));

    // Exact resolution
    assert!(registry.get("market_quote").is_some());

    // tantra_ prefix resolution
    assert!(registry.get("tantra_market_quote").is_some());
    assert!(registry.get("tantra_research_calc").is_some());

    // Canonical alias resolution
    assert!(registry.get("tantra_ticker_price").is_some());
    assert!(registry.get("tantra_calculate").is_some());
    assert!(registry.get("tantra_screener").is_some());

    // Snapshot also supports resolution
    let snapshot = registry.snapshot();
    assert!(snapshot.get("tantra_market_quote").is_some());
    assert!(snapshot.get("tantra_calculate").is_some());
}

/// Drains session events until `TurnEnded`, collecting the streamed assistant text.
async fn drain_until_turn_end(
    events: &mut tantra_agent_chat::SessionEvents,
) -> (String, TurnOutcome) {
    let mut text = String::new();
    let mut outcome = None;
    while let Ok(Some(event)) = timeout(Duration::from_secs(5), events.recv()).await {
        match event {
            SessionEvent::TextDelta { text: delta, .. } => text.push_str(&delta),
            SessionEvent::TurnEnded { outcome: out, .. } => {
                outcome = Some(out);
                break;
            }
            _ => {}
        }
    }
    (text, outcome.unwrap_or_else(|| panic!("turn never ended")))
}

fn png_attachment() -> tantra_agent_llm::MessagePart {
    tantra_agent_llm::MessagePart::Image {
        mime_type: "image/png".to_string(),
        data: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGNgYGBgAAAABQABh6FO1AAAAABJRU5ErkJggg==".to_string(),
    }
}

#[tokio::test]
async fn test_image_attachments_flow_to_vision_provider() {
    // Gap 0009 (T17.2): image attachment parts ride through the turn into the LLM boundary for
    // vision-capable providers.
    let vision = Arc::new(ScriptedFakeLlm::new("vision-model").with_vision());
    vision.push_text_response("I can see the chart crop.");

    let store = Arc::new(InMemorySessionStore::new());
    let registry = ToolRegistry::new();
    let agent = ChatAgent::builder(vision.clone(), registry, store.clone())
        .build()
        .expect("agent");

    let scope = SessionScope::new("vision-test", RuntimeMode::Paper);
    let (handle, mut events) = agent.open(SessionSelector::New(scope)).await.expect("open");

    handle
        .submit_turn("Analyze this chart", vec![png_attachment()], None)
        .await
        .expect("submit");

    let (text, outcome) = drain_until_turn_end(&mut events).await;
    assert_eq!(outcome, TurnOutcome::Completed);
    assert_eq!(text, "I can see the chart crop.");

    // The vision provider received exactly one request carrying the image part.
    let served = vision.served_requests();
    assert_eq!(served.len(), 1);
    assert!(served[0].has_image_parts());
    assert!(matches!(
        &served[0].messages[0].parts[0],
        tantra_agent_llm::MessagePart::Image { mime_type, .. } if mime_type == "image/png"
    ));

    // The image part also persisted into the session history faithfully.
    let history = handle.history().await.expect("history");
    let user_msg = history
        .iter()
        .find(|pm| pm.role == "user")
        .expect("user message persisted");
    assert!(user_msg.content.get("attachments").is_some());
}

#[tokio::test]
async fn test_text_only_provider_answers_capability_notice_for_images() {
    // Gap 0009 (T17.2): a text-only provider MUST answer image attachments with the honest
    // capability notice (streamed as text so it lands in the chat reply), never silently drop
    // the bytes.
    let text_only = Arc::new(ScriptedFakeLlm::new("textonly-model"));
    text_only.push_text_response("must not be reached");

    let store = Arc::new(InMemorySessionStore::new());
    let registry = ToolRegistry::new();
    let agent = ChatAgent::builder(text_only.clone(), registry, store)
        .build()
        .expect("agent");

    let scope = SessionScope::new("vision-test", RuntimeMode::Paper);
    let (handle, mut events) = agent.open(SessionSelector::New(scope)).await.expect("open");

    handle
        .submit_turn("Read this chart", vec![png_attachment()], None)
        .await
        .expect("submit");

    let (text, outcome) = drain_until_turn_end(&mut events).await;
    assert!(text.contains("textonly-model does not support image attachments"));
    match outcome {
        TurnOutcome::Failed { error } => {
            assert_eq!(error.kind, "capability");
            assert!(error.message.contains("does not support image attachments"));
        }
        other => panic!("expected a capability failure, got {other:?}"),
    }
    // The text-only provider was never invoked (the turn gates before the LLM boundary).
    assert!(text_only.served_requests().is_empty());
}

#[tokio::test]
async fn test_per_turn_provider_override_routes_the_turn() {
    // Gap 0009 (T17.2): a per-turn provider override routes the whole turn through the
    // override provider; the session's default provider stays untouched.
    let default = Arc::new(ScriptedFakeLlm::new("default-llm"));
    default.push_text_response("default answer");
    let override_provider = Arc::new(ScriptedFakeLlm::new("override-llm"));
    override_provider.push_text_response("override answer");

    let store = Arc::new(InMemorySessionStore::new());
    let registry = ToolRegistry::new();
    let agent = ChatAgent::builder(default.clone(), registry, store)
        .build()
        .expect("agent");

    let scope = SessionScope::new("override-test", RuntimeMode::Paper);
    let (handle, mut events) = agent.open(SessionSelector::New(scope)).await.expect("open");

    handle
        .submit_turn(
            "Use the override",
            Vec::new(),
            Some(override_provider.clone() as Arc<dyn tantra_agent_llm::LlmProvider>),
        )
        .await
        .expect("submit");

    let (text, outcome) = drain_until_turn_end(&mut events).await;
    assert_eq!(outcome, TurnOutcome::Completed);
    assert_eq!(text, "override answer");

    assert_eq!(override_provider.served_requests().len(), 1);
    assert!(default.served_requests().is_empty());
}
