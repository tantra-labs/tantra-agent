use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use agent_client_protocol::v1::schema::StopReason as AcpStop;
use futures::{SinkExt, StreamExt};
use serde_json::json;
use tantra_agent_acp::{
    AcpError, AgentHost, ClientEvent, ClientOptions, StaticCatalog, connect_ws, serve,
};
use tantra_agent_chat::{FakeChatTool, InMemorySessionStore, ToolRegistry, ToolRisk, TurnConfig};
use tantra_agent_llm::{
    ChatMessage, ChatStreamEvent, LlmToolCall, ScriptedFakeLlm, StopReason, Usage,
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};

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

fn host(a: Arc<ScriptedFakeLlm>, risk: ToolRisk) -> Arc<AgentHost> {
    let tool = FakeChatTool::new("quote", risk);
    tool.set_result(Ok(json!({"ok": true})));
    let mut reg = ToolRegistry::new();
    reg.register_all(vec![Arc::new(tool)]);
    Arc::new(AgentHost {
        system_prompt: "test".into(),
        tools: Arc::new(reg.snapshot()),
        store: Arc::new(InMemorySessionStore::new()),
        catalog: Arc::new(StaticCatalog::new().with_model("fake-a", "Fake A", a)),
        turn_config: TurnConfig::default(),
        max_sessions: 4,
    })
}

/// Accept one WS connection and serve `host` over it; returns (url, auth header receiver).
#[allow(clippy::result_large_err)]
async fn spawn_server(host: Arc<AgentHost>) -> (String, oneshot::Receiver<String>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (auth_tx, auth_rx) = oneshot::channel::<String>();
    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut auth_tx = Some(auth_tx);
        let ws =
            tokio_tungstenite::accept_hdr_async(stream, move |req: &Request, resp: Response| {
                let v = req
                    .headers()
                    .get("authorization")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("")
                    .to_string();
                if let Some(tx) = auth_tx.take() {
                    let _ = tx.send(v);
                }
                Ok(resp)
            })
            .await
            .unwrap();
        let (mut sink, mut stream) = ws.split();
        let (agent_io, bridge_io) = tokio::io::duplex(256 * 1024);
        let (a_r, a_w) = tokio::io::split(agent_io);
        let (b_r, mut b_w) = tokio::io::split(bridge_io);
        tokio::spawn(async move {
            let _ = serve(host, a_r, a_w).await;
        });
        tokio::spawn(async move {
            while let Some(Ok(m)) = stream.next().await {
                if let Message::Text(t) = m {
                    let mut l = t.replace(['\r', '\n'], " ");
                    l.push('\n');
                    if b_w.write_all(l.as_bytes()).await.is_err() {
                        break;
                    }
                }
            }
        });
        let mut rd = BufReader::new(b_r);
        let mut line = String::new();
        loop {
            line.clear();
            match rd.read_line(&mut line).await {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    let t = line.trim();
                    if !t.is_empty() && sink.send(Message::Text(t.to_string())).await.is_err() {
                        break;
                    }
                }
            }
        }
    });
    (format!("ws://{addr}"), auth_rx)
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

#[tokio::test]
async fn ws_text_turn_with_bearer() {
    let llm = Arc::new(ScriptedFakeLlm::new("fake-a"));
    llm.push_stream(text_stream("Hello over ws"));
    let (url, auth_rx) = spawn_server(host(llm, ToolRisk::ReadOnly)).await;
    let (client, mut rx) = connect_ws(&url, Some("tok123"), ClientOptions::default())
        .await
        .unwrap();
    client.prompt("hi".into()).await.unwrap();
    let evs = collect(&mut rx).await;
    let text: String = evs
        .iter()
        .filter_map(|e| match e {
            ClientEvent::Text(t) => Some(t.as_str()),
            _ => None,
        })
        .collect();
    assert!(text.contains("Hello over ws"), "{evs:?}");
    assert!(matches!(
        evs.last(),
        Some(ClientEvent::TurnEnded(AcpStop::EndTurn))
    ));
    assert_eq!(auth_rx.await.unwrap(), "Bearer tok123");
}

#[tokio::test]
async fn idle_client_answers_server_ping() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
        ws.send(Message::Ping(b"hb".to_vec())).await.unwrap();
        tokio::time::timeout(Duration::from_secs(2), async {
            while let Some(Ok(m)) = ws.next().await {
                if let Message::Pong(p) = m {
                    return Some(p);
                }
            }
            None
        })
        .await
    });
    // The handshake never completes (no ACP server), so run connect in the background.
    let url = format!("ws://{addr}");
    let _client = tokio::spawn(async move {
        let _ = connect_ws(&url, None, ClientOptions::default()).await;
    });
    let got = server.await.unwrap().expect("no pong within 2s");
    assert_eq!(got, Some(b"hb".to_vec()));
}

#[tokio::test]
async fn rejects_non_ws_scheme() {
    let r = connect_ws("http://x", None, ClientOptions::default()).await;
    assert!(matches!(r, Err(AcpError::Protocol(_))));
}

#[tokio::test]
async fn approve_flag_toggles_at_runtime() {
    let llm = Arc::new(ScriptedFakeLlm::new("fake-a"));
    llm.push_stream(tool_stream("quote"));
    llm.push_stream(text_stream("first"));
    llm.push_stream(tool_stream("quote"));
    llm.push_stream(text_stream("second"));
    let (url, _auth) = spawn_server(host(llm, ToolRisk::LocalWrite)).await;
    let flag = Arc::new(AtomicBool::new(false));
    let opts = ClientOptions {
        approve: Some(flag.clone()),
        ..Default::default()
    };
    let (client, mut rx) = connect_ws(&url, None, opts).await.unwrap();

    client.prompt("one".into()).await.unwrap();
    let evs = collect(&mut rx).await;
    assert!(
        evs.iter()
            .any(|e| matches!(e, ClientEvent::ToolFinished { ok: false, .. })),
        "{evs:?}"
    );

    flag.store(true, Ordering::Relaxed);
    client.prompt("two".into()).await.unwrap();
    let evs = collect(&mut rx).await;
    assert!(
        evs.iter()
            .any(|e| matches!(e, ClientEvent::ToolFinished { ok: true, .. })),
        "{evs:?}"
    );
}
