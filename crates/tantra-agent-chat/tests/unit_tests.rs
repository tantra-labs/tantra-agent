//! Comprehensive test suite for tantra-agent-chat (§5, Phase A).

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use serde_json::json;
use tokio::time::timeout;

use tantra_agent_chat::{
    ChatAgent, ChatConfig, ChatError, ConfirmOverride, FakeChatTool, InMemorySessionStore,
    PermissionDecision, SessionEvent, SessionSelector, ToolRegistry, ToolRisk, TurnOutcome,
    run_turn_headless,
};
use tantra_agent_core::{
    CodeDiffHunk, DataReferenceStore, DisplayBlock, NewSession, PersistedMessage, RuntimeMode,
    SessionId, SessionScope, SessionStore, StoreErrorKind, TantraResultEnvelope,
};
use tantra_agent_llm::{
    ChatMessage, ChatStreamEvent, LlmToolCall, ScriptedFakeLlm, StopReason, Usage,
};

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
        .expect("agent build failed");

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

#[tokio::test]
async fn test_turn_streams_thinking_usage_raw_payloads_and_code_diff() {
    let fake_llm = Arc::new(ScriptedFakeLlm::new("test-claude"));

    let call = LlmToolCall {
        id: "call_diff_1".to_string(),
        name: "strategy_patch".to_string(),
        arguments: json!({ "symbol": "NIFTY", "fast": 9 }).to_string(),
    };
    fake_llm.push_stream(vec![
        ChatStreamEvent::ThinkingDelta {
            text: "Inspecting EMA params...".into(),
        },
        ChatStreamEvent::ToolCallReady {
            call_id: "call_diff_1".into(),
            name: "strategy_patch".into(),
            arguments: json!({ "symbol": "NIFTY", "fast": 9 }),
            raw_arguments: call.arguments.clone(),
            is_malformed: false,
        },
        ChatStreamEvent::Usage(Usage {
            input_tokens: 12,
            output_tokens: 8,
            cache_read_tokens: Some(4),
            cache_write_tokens: None,
        }),
        ChatStreamEvent::MessageEnd {
            stop_reason: StopReason::ToolUse,
            message: ChatMessage::assistant_with_tools("", vec![call]),
            usage: Usage::default(),
        },
    ]);
    fake_llm.push_stream(vec![
        ChatStreamEvent::TextDelta {
            text: "Patch ready.".into(),
        },
        ChatStreamEvent::MessageEnd {
            stop_reason: StopReason::EndTurn,
            message: ChatMessage::assistant("Patch ready."),
            usage: Usage {
                input_tokens: 30,
                output_tokens: 10,
                cache_read_tokens: None,
                cache_write_tokens: None,
            },
        },
    ]);

    let store = Arc::new(InMemorySessionStore::new());
    let mut registry = ToolRegistry::new();
    let diff_tool = Arc::new(FakeChatTool::new("strategy_patch", ToolRisk::ReadOnly));
    let env = TantraResultEnvelope::ok("EMA patch", json!({ "fast": 9 })).with_blocks(vec![
        DisplayBlock::CodeDiff {
            title: "EMA Entry v2".into(),
            lang: "python".into(),
            code: "fast = 9\n".into(),
            hunks: vec![CodeDiffHunk {
                id: "h1".into(),
                header: "@@ -1,1 +1,1 @@".into(),
                lines: vec!["-fast = 12".into(), "+fast = 9".into()],
            }],
        },
    ]);
    diff_tool.set_result(Ok(serde_json::to_value(env).unwrap()));
    registry.register(diff_tool);

    let agent = ChatAgent::builder(fake_llm, registry, store.clone())
        .build()
        .expect("agent build failed");
    let (handle, mut events) = agent
        .open(SessionSelector::New(SessionScope::new(
            "trace-test",
            RuntimeMode::Paper,
        )))
        .await
        .expect("open");

    handle.submit("Tune fast EMA").await.expect("submit");

    let mut thoughts = String::new();
    let mut usage_events = Vec::new();
    let mut seen_raw_input = None;
    let mut seen_raw_output = None;
    let mut seen_blocks = Vec::new();
    let mut final_usage = Usage::default();

    while let Ok(Some(ev)) = timeout(Duration::from_secs(5), events.recv()).await {
        match ev {
            SessionEvent::ThinkingDelta { text, .. } => thoughts.push_str(&text),
            SessionEvent::Usage { usage, .. } => usage_events.push(usage),
            SessionEvent::ToolStarted { raw_input, .. } => seen_raw_input = raw_input,
            SessionEvent::ToolFinished {
                raw_output, blocks, ..
            } => {
                seen_raw_output = raw_output;
                seen_blocks = blocks;
            }
            SessionEvent::TurnEnded { outcome, usage, .. } => {
                assert_eq!(outcome, TurnOutcome::Completed);
                final_usage = usage;
                break;
            }
            _ => {}
        }
    }

    assert_eq!(thoughts, "Inspecting EMA params...");
    assert_eq!(usage_events.len(), 2);
    assert_eq!(usage_events[0].input_tokens, 12);
    assert_eq!(usage_events[1].input_tokens, 30);
    assert_eq!(final_usage.input_tokens, 42);
    assert_eq!(final_usage.output_tokens, 18);
    assert_eq!(final_usage.cache_read_tokens, Some(4));
    assert_eq!(
        seen_raw_input,
        Some(json!({ "symbol": "NIFTY", "fast": 9 }))
    );
    assert_eq!(seen_raw_output.as_ref().unwrap()["data"]["fast"], 9);
    assert!(matches!(
        seen_blocks.as_slice(),
        [DisplayBlock::CodeDiff { title, lang, .. }] if title == "EMA Entry v2" && lang == "python"
    ));
}

#[tokio::test]
async fn test_in_memory_session_store_get_archive_delete_and_resume_preserves_scope() {
    let store = Arc::new(InMemorySessionStore::new());
    let id = SessionId::new();
    store
        .create(NewSession {
            id,
            profile: "workspace-alpha".to_string(),
            mode: RuntimeMode::Paper,
            provider: "anthropic".to_string(),
            model: "claude-sonnet".to_string(),
            title: Some("Alpha Thread".to_string()),
        })
        .await
        .expect("create");

    let fetched = store
        .get(id)
        .await
        .expect("get")
        .expect("session should exist");
    assert_eq!(fetched.id, id);
    assert_eq!(fetched.profile, "workspace-alpha");
    assert_eq!(fetched.mode, RuntimeMode::Paper);
    assert!(!fetched.archived);

    assert!(
        store
            .get(SessionId::new())
            .await
            .expect("get missing")
            .is_none()
    );

    // Appending updates `updated_at`
    let prev_updated = fetched.updated_at;
    tokio::time::sleep(Duration::from_millis(2)).await;
    store
        .append(
            id,
            &[PersistedMessage {
                seq: 1,
                role: "user".to_string(),
                content: json!({ "text": "hello" }),
                request_hash: None,
                input_tokens: None,
                output_tokens: None,
                created_at: Utc::now(),
            }],
        )
        .await
        .expect("append");
    let after_append = store.get(id).await.unwrap().unwrap();
    assert!(after_append.updated_at >= prev_updated);

    // Resuming preserves stored `SessionScope` instead of hardcoding default/Live
    let fake_llm = Arc::new(ScriptedFakeLlm::new("test-claude"));
    let agent = ChatAgent::builder(fake_llm, ToolRegistry::new(), store.clone())
        .build()
        .expect("agent");
    let (_handle, mut events) = agent
        .open(SessionSelector::Resume(id))
        .await
        .expect("resume should succeed");
    let opened = timeout(Duration::from_secs(5), events.recv())
        .await
        .expect("timeout")
        .expect("opened event");
    match opened {
        SessionEvent::SessionOpened {
            session,
            resumed,
            history,
            profile,
            ..
        } => {
            assert_eq!(session, Some(id));
            assert!(resumed);
            assert_eq!(history, 1);
            assert_eq!(
                profile,
                SessionScope::new("workspace-alpha", RuntimeMode::Paper)
            );
        }
        other => panic!("expected SessionOpened, got {other:?}"),
    }

    // Resuming an unknown session fails explicitly with NotFound
    let missing_id = SessionId::new();
    match agent.open(SessionSelector::Resume(missing_id)).await {
        Err(ChatError::Store(err)) => assert_eq!(err.kind, StoreErrorKind::NotFound),
        Ok(_) => panic!("resuming missing session must fail"),
        Err(other) => panic!("expected Store NotFound error, got {other:?}"),
    }

    // Archiving removes session from active `most_recent` lookup
    store.set_archived(id, true).await.expect("archive");
    assert!(store.get(id).await.unwrap().unwrap().archived);
    assert!(
        store
            .most_recent(&SessionScope::new("workspace-alpha", RuntimeMode::Paper))
            .await
            .unwrap()
            .is_none()
    );
    store.set_archived(id, false).await.expect("unarchive");
    assert!(
        store
            .most_recent(&SessionScope::new("workspace-alpha", RuntimeMode::Paper))
            .await
            .unwrap()
            .is_some()
    );

    // Deleting removes session + messages cleanly and missing delete returns NotFound
    store.delete(id).await.expect("delete");
    assert!(store.get(id).await.unwrap().is_none());
    assert!(store.messages(id).await.unwrap().is_empty());
    assert_eq!(
        store.delete(id).await.unwrap_err().kind,
        StoreErrorKind::NotFound
    );
    assert_eq!(
        store.set_archived(id, true).await.unwrap_err().kind,
        StoreErrorKind::NotFound
    );
}

#[tokio::test]
async fn test_chart_series_ref_resolved_in_turn() {
    let fake_llm = Arc::new(ScriptedFakeLlm::new("test-claude"));
    fake_llm.push_tool_call_response(
        "call_chart",
        "backtest_chart",
        json!({"symbol": "RELIANCE"}),
    );
    fake_llm.push_text_response("Here is your equity curve.");

    let ref_store = Arc::new(DataReferenceStore::new());
    let uri = ref_store.export(
        "eq_reliance",
        json!([[1_700_000_000, "100.00"], [1_700_003_600, "104.25"]]),
    );

    let store = Arc::new(InMemorySessionStore::new());
    let mut registry = ToolRegistry::new();
    let chart_tool = Arc::new(FakeChatTool::new("backtest_chart", ToolRisk::ReadOnly));
    let env = TantraResultEnvelope::ok("Chart ready", json!({ "ok": true })).with_block(
        DisplayBlock::Chart {
            title: "RELIANCE Equity".to_string(),
            chart_type: "line".to_string(),
            series_ref: uri.clone(),
            series: Vec::new(),
        },
    );
    chart_tool.set_result(Ok(serde_json::to_value(env).unwrap()));
    registry.register(chart_tool);

    let agent = ChatAgent::builder(fake_llm, registry, store)
        .ref_reader(ref_store)
        .build()
        .expect("agent build");
    let (_outcome, events) = run_turn_headless(
        &agent,
        SessionSelector::New(SessionScope::new("chart-test", RuntimeMode::Paper)),
        "Plot equity",
    )
    .await
    .expect("run turn");

    let chart_blocks: Vec<&DisplayBlock> = events
        .iter()
        .filter_map(|ev| match ev {
            SessionEvent::ToolFinished { blocks, .. } => Some(blocks.iter()),
            _ => None,
        })
        .flatten()
        .collect();
    assert_eq!(chart_blocks.len(), 1);
    assert!(matches!(
        chart_blocks[0],
        DisplayBlock::Chart {
            title,
            series_ref,
            series,
            ..
        } if title == "RELIANCE Equity"
            && series_ref == &uri
            && series.as_slice() == [
                (1_700_000_000, "100.00".to_string()),
                (1_700_003_600, "104.25".to_string()),
            ]
    ));
}
