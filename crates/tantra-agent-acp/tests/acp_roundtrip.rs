use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use tantra_agent_acp::{
    AcpClient, AgentHost, ClientEvent, ClientOptions, StaticCatalog, connect_embedded,
};
use tantra_agent_chat::{FakeChatTool, InMemorySessionStore, ToolRegistry, ToolRisk, TurnConfig};
use tantra_agent_core::{DataReferenceStore, DisplayBlock, MetricItem, TantraResultEnvelope};
use tantra_agent_llm::{
    ChatMessage, ChatStreamEvent, LlmToolCall, ScriptedFakeLlm, StopReason, Usage,
};
use tokio::sync::mpsc;

use agent_client_protocol::v1::schema::StopReason as AcpStop;

fn end(msg: ChatMessage, reason: StopReason) -> ChatStreamEvent {
    ChatStreamEvent::MessageEnd {
        stop_reason: reason,
        message: msg,
        usage: Usage::default(),
    }
}

fn text_stream(text: &str) -> Vec<ChatStreamEvent> {
    vec![
        ChatStreamEvent::TextDelta { text: text.into() },
        end(ChatMessage::assistant(text), StopReason::EndTurn),
    ]
}

fn tool_stream(tool: &str) -> Vec<ChatStreamEvent> {
    let args = "{}".to_string();
    vec![
        ChatStreamEvent::ToolCallReady {
            call_id: "c1".into(),
            name: tool.into(),
            arguments: json!({}),
            raw_arguments: args.clone(),
            is_malformed: false,
        },
        end(
            ChatMessage::assistant_with_tools(
                "",
                vec![LlmToolCall {
                    id: "c1".into(),
                    name: tool.into(),
                    arguments: args,
                }],
            ),
            StopReason::ToolUse,
        ),
    ]
}

fn host(a: Arc<ScriptedFakeLlm>, b: Arc<ScriptedFakeLlm>, risk: ToolRisk) -> Arc<AgentHost> {
    let tool = FakeChatTool::new("quote", risk);
    let env =
        TantraResultEnvelope::ok("q", json!({})).with_blocks(vec![DisplayBlock::MetricGrid {
            items: vec![MetricItem {
                label: "Sharpe".into(),
                value: "1.44".into(),
            }],
        }]);
    tool.set_result(Ok(serde_json::to_value(env).unwrap()));
    let mut reg = ToolRegistry::new();
    reg.register_all(vec![Arc::new(tool)]);
    Arc::new(AgentHost {
        system_prompt: "test".into(),
        tools: Arc::new(reg.snapshot()),
        store: Arc::new(InMemorySessionStore::new()),
        catalog: Arc::new(
            StaticCatalog::new()
                .with_model("fake-a", "Fake A", a)
                .with_model("fake-b", "Fake B", b),
        ),
        turn_config: TurnConfig::default(),
        max_sessions: 4,
    })
}

fn llms() -> (Arc<ScriptedFakeLlm>, Arc<ScriptedFakeLlm>) {
    (
        Arc::new(ScriptedFakeLlm::new("fake-a")),
        Arc::new(ScriptedFakeLlm::new("fake-b")),
    )
}

async fn collect(rx: &mut mpsc::Receiver<ClientEvent>) -> Vec<ClientEvent> {
    tokio::time::timeout(Duration::from_secs(10), async {
        let mut out = Vec::new();
        while let Some(ev) = rx.recv().await {
            let done = matches!(ev, ClientEvent::TurnEnded(_) | ClientEvent::Error(_));
            out.push(ev);
            if done {
                break;
            }
        }
        out
    })
    .await
    .expect("timed out")
}

fn has_text(evs: &[ClientEvent], needle: &str) -> bool {
    evs.iter()
        .any(|e| matches!(e, ClientEvent::Text(t) if t.contains(needle)))
}

async fn run(client: &AcpClient, rx: &mut mpsc::Receiver<ClientEvent>) -> Vec<ClientEvent> {
    client.prompt("hi".into()).await.unwrap();
    collect(rx).await
}

#[tokio::test]
async fn lists_models_and_default() {
    let (a, b) = llms();
    let (client, _rx) = connect_embedded(host(a, b, ToolRisk::ReadOnly), ClientOptions::default())
        .await
        .unwrap();
    let ids: Vec<_> = client.models().iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, ["fake-a", "fake-b"]);
    assert_eq!(client.default_model(), "fake-a");
    assert_eq!(client.model(), "fake-a");
}

#[tokio::test]
async fn text_turn() {
    let (a, b) = llms();
    a.push_stream(text_stream("Hello"));
    let (client, mut rx) =
        connect_embedded(host(a, b, ToolRisk::ReadOnly), ClientOptions::default())
            .await
            .unwrap();
    let evs = run(&client, &mut rx).await;
    assert!(has_text(&evs, "Hello"), "{evs:?}");
    assert!(matches!(
        evs.last(),
        Some(ClientEvent::TurnEnded(AcpStop::EndTurn))
    ));
}

#[tokio::test]
async fn tool_turn_carries_blocks() {
    let (a, b) = llms();
    a.push_stream(tool_stream("quote"));
    a.push_stream(text_stream("done"));
    let (client, mut rx) =
        connect_embedded(host(a, b, ToolRisk::ReadOnly), ClientOptions::default())
            .await
            .unwrap();
    let evs = run(&client, &mut rx).await;
    let started = evs
        .iter()
        .position(|e| matches!(e, ClientEvent::ToolStarted { tool, .. } if tool == "quote"))
        .expect("started");
    let finished = evs
        .iter()
        .position(|e| matches!(e, ClientEvent::ToolFinished { ok: true, blocks, .. } if blocks.len() == 1))
        .expect("finished");
    let done = evs
        .iter()
        .position(|e| matches!(e, ClientEvent::Text(t) if t.contains("done")))
        .expect("text");
    assert!(started < finished && finished < done, "{evs:?}");
    assert!(matches!(
        evs.last(),
        Some(ClientEvent::TurnEnded(AcpStop::EndTurn))
    ));
}

#[tokio::test]
async fn model_selection_in_options() {
    let (a, b) = llms();
    b.push_stream(text_stream("from-b"));
    let opts = ClientOptions {
        model: Some("fake-b".into()),
        ..Default::default()
    };
    let (client, mut rx) = connect_embedded(host(a.clone(), b.clone(), ToolRisk::ReadOnly), opts)
        .await
        .unwrap();
    assert_eq!(client.model(), "fake-b");
    let evs = run(&client, &mut rx).await;
    assert!(has_text(&evs, "from-b"), "{evs:?}");

    let bad = ClientOptions {
        model: Some("nope".into()),
        ..Default::default()
    };
    assert!(
        connect_embedded(host(a, b, ToolRisk::ReadOnly), bad)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn permission_fail_closed_and_approved() {
    for (auto, expect_ok) in [(false, false), (true, true)] {
        let (a, b) = llms();
        a.push_stream(tool_stream("quote"));
        a.push_stream(text_stream("done"));
        let opts = ClientOptions {
            auto_approve: auto,
            ..Default::default()
        };
        let (client, mut rx) = connect_embedded(host(a, b, ToolRisk::LocalWrite), opts)
            .await
            .unwrap();
        let evs = run(&client, &mut rx).await;
        assert!(
            evs.iter()
                .any(|e| matches!(e, ClientEvent::ToolFinished { ok, .. } if *ok == expect_ok)),
            "auto={auto}: {evs:?}"
        );
    }
}

#[tokio::test]
async fn set_model_switches_provider() {
    let (a, b) = llms();
    b.push_stream(text_stream("switched"));
    let (mut client, mut rx) =
        connect_embedded(host(a, b, ToolRisk::ReadOnly), ClientOptions::default())
            .await
            .unwrap();
    client.set_model("fake-b").await.unwrap();
    assert_eq!(client.model(), "fake-b");
    let evs = run(&client, &mut rx).await;
    assert!(has_text(&evs, "switched"), "{evs:?}");
    assert!(client.set_model("nope").await.is_err());
}

#[tokio::test]
async fn chart_series_ref_resolved_over_acp() {
    let (a, b) = llms();
    a.push_stream(tool_stream("chart_tool"));
    a.push_stream(text_stream("plotted"));

    let ref_store = Arc::new(DataReferenceStore::new());
    let uri = ref_store.export(
        "acp_eq",
        json!([[1_700_000_000, "100.00"], [1_700_000_060, "101.75"]]),
    );
    let tool = FakeChatTool::new("chart_tool", ToolRisk::ReadOnly);
    let env = TantraResultEnvelope::ok("chart", json!({})).with_block(DisplayBlock::Chart {
        title: "Equity".into(),
        chart_type: "line".into(),
        series_ref: uri.clone(),
        series: Vec::new(),
    });
    tool.set_result(Ok(serde_json::to_value(env).unwrap()));
    let mut reg = ToolRegistry::new();
    reg.register_all(vec![Arc::new(tool)]);

    let acp_host = Arc::new(AgentHost {
        system_prompt: "test".into(),
        tools: Arc::new(reg.snapshot()),
        store: Arc::new(InMemorySessionStore::new()),
        catalog: Arc::new(
            StaticCatalog::new()
                .with_model("fake-a", "Fake A", a)
                .with_model("fake-b", "Fake B", b),
        ),
        turn_config: TurnConfig::default().with_ref_reader(ref_store),
        max_sessions: 4,
    });

    let (client, mut rx) = connect_embedded(acp_host, ClientOptions::default())
        .await
        .unwrap();
    let evs = run(&client, &mut rx).await;
    let blocks = evs
        .iter()
        .find_map(|e| match e {
            ClientEvent::ToolFinished {
                ok: true, blocks, ..
            } => Some(blocks),
            _ => None,
        })
        .expect("ToolFinished");
    assert!(matches!(
        blocks.as_slice(),
        [DisplayBlock::Chart {
            series_ref,
            series,
            ..
        }] if series_ref == &uri
            && series.as_slice() == [
                (1_700_000_000, "100.00".to_string()),
                (1_700_000_060, "101.75".to_string()),
            ]
    ));
}
