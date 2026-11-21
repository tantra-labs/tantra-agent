//! ACP client side used by the TUI/CLI: typed events over an ACP connection.

use std::any::Any;
use std::sync::Arc;

use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::v1::schema::*;
use agent_client_protocol::v1::{ConnectionTo, Responder};
use agent_client_protocol::{Agent, ByteStreams, Client, Error, UntypedMessage};
use serde_json::{Value, json};
use tantra_agent_core::DisplayBlock;
use tokio::sync::{mpsc, oneshot};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

use crate::agent::{AgentHost, serve};
use crate::catalog::ModelInfo;
use crate::error::AcpError;
use crate::meta::{read_tantra, tantra_meta};

#[derive(Debug, Clone)]
pub enum ClientEvent {
    Text(String),
    Thought(String),
    ToolStarted {
        call_id: String,
        tool: String,
        title: String,
    },
    ToolFinished {
        call_id: String,
        tool: String,
        ok: bool,
        elapsed_ms: u64,
        blocks: Vec<DisplayBlock>,
        detail: String,
    },
    TurnEnded(StopReason),
    Error(String),
}

#[derive(Debug, Clone, Default)]
pub struct ClientOptions {
    pub auto_approve: bool,
    pub model: Option<String>,
    pub workspace: Option<String>,
}

enum Command {
    Prompt(String),
    Cancel,
    SetModel(String, oneshot::Sender<Result<String, AcpError>>),
}

struct Handshake {
    models: Vec<ModelInfo>,
    default_model: String,
    model: String,
}

/// Handle to a background ACP client connection with one session.
pub struct AcpClient {
    tx: mpsc::Sender<Command>,
    models: Vec<ModelInfo>,
    default_model: String,
    model: String,
}

impl AcpClient {
    pub fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    pub fn default_model(&self) -> &str {
        &self.default_model
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    /// Enqueue a prompt; events and `TurnEnded` arrive on the event receiver.
    pub async fn prompt(&self, text: String) -> Result<(), AcpError> {
        self.tx
            .send(Command::Prompt(text))
            .await
            .map_err(|_| AcpError::Closed)
    }

    pub async fn cancel(&self) {
        let _ = self.tx.send(Command::Cancel).await;
    }

    pub async fn set_model(&mut self, id: &str) -> Result<(), AcpError> {
        let (otx, orx) = oneshot::channel();
        self.tx
            .send(Command::SetModel(id.to_string(), otx))
            .await
            .map_err(|_| AcpError::Closed)?;
        let model = orx.await.map_err(|_| AcpError::Closed)??;
        self.model = model;
        Ok(())
    }

    pub async fn connect(
        reader: impl tokio::io::AsyncRead + Send + Unpin + 'static,
        writer: impl tokio::io::AsyncWrite + Send + Unpin + 'static,
        opts: ClientOptions,
    ) -> Result<(AcpClient, mpsc::Receiver<ClientEvent>), AcpError> {
        connect_inner(reader, writer, opts, None).await
    }
}

fn parse_blocks(meta: &Option<Meta>) -> Vec<DisplayBlock> {
    read_tantra(meta)
        .and_then(|v| v.get("blocks"))
        .and_then(|b| serde_json::from_value(b.clone()).ok())
        .unwrap_or_default()
}

fn meta_tool(meta: &Option<Meta>) -> String {
    read_tantra(meta)
        .and_then(|v| v.get("tool"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn first_text(content: &Option<Vec<ToolCallContent>>) -> String {
    content
        .as_ref()
        .and_then(|c| serde_json::to_value(c).ok())
        .and_then(|v| {
            v.get(0)
                .and_then(|i| i.get("content"))
                .and_then(|c| c.get("text"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_default()
}

fn map_notification(n: SessionNotification) -> Option<ClientEvent> {
    match n.update {
        SessionUpdate::AgentMessageChunk(c) => match c.content {
            ContentBlock::Text(t) => Some(ClientEvent::Text(t.text)),
            _ => None,
        },
        SessionUpdate::AgentThoughtChunk(c) => match c.content {
            ContentBlock::Text(t) => Some(ClientEvent::Thought(t.text)),
            _ => None,
        },
        SessionUpdate::ToolCall(tc) => Some(ClientEvent::ToolStarted {
            call_id: tc.tool_call_id.0.to_string(),
            tool: meta_tool(&tc.meta),
            title: tc.title,
        }),
        SessionUpdate::ToolCallUpdate(u) => {
            let elapsed_ms = read_tantra(&u.meta)
                .and_then(|v| v.get("elapsedMs"))
                .and_then(Value::as_u64)
                .unwrap_or(0);
            Some(ClientEvent::ToolFinished {
                call_id: u.tool_call_id.0.to_string(),
                tool: meta_tool(&u.meta),
                ok: matches!(u.fields.status, Some(ToolCallStatus::Completed)),
                elapsed_ms,
                blocks: parse_blocks(&u.meta),
                detail: first_text(&u.fields.content),
            })
        }
        _ => None,
    }
}

async fn handshake(
    cx: &ConnectionTo<Agent>,
    opts: &ClientOptions,
) -> Result<(Handshake, SessionId), Error> {
    let init = cx
        .send_request(InitializeRequest::new(ProtocolVersion::V1))
        .block_task()
        .await?;
    let tantra = read_tantra(&init.meta);
    let models: Vec<ModelInfo> = tantra
        .and_then(|v| v.get("models"))
        .and_then(|m| serde_json::from_value(m.clone()).ok())
        .unwrap_or_default();
    let default_model = tantra
        .and_then(|v| v.get("defaultModel"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    let mut m = serde_json::Map::new();
    if let Some(model) = &opts.model {
        m.insert("model".into(), json!(model));
    }
    if let Some(ws) = &opts.workspace {
        m.insert("workspace".into(), json!(ws));
    }
    let cwd = opts.workspace.clone().unwrap_or_else(|| ".".to_string());
    let mut req = NewSessionRequest::new(cwd);
    if !m.is_empty() {
        req = req.meta(tantra_meta(Value::Object(m)));
    }
    let ns = cx.send_request(req).block_task().await?;
    let model = read_tantra(&ns.meta)
        .and_then(|v| v.get("model"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| default_model.clone());
    Ok((
        Handshake {
            models,
            default_model,
            model,
        },
        ns.session_id,
    ))
}

async fn connect_inner(
    reader: impl tokio::io::AsyncRead + Send + Unpin + 'static,
    writer: impl tokio::io::AsyncWrite + Send + Unpin + 'static,
    opts: ClientOptions,
    keepalive: Option<Box<dyn Any + Send>>,
) -> Result<(AcpClient, mpsc::Receiver<ClientEvent>), AcpError> {
    let (ev_tx, ev_rx) = mpsc::channel::<ClientEvent>(256);
    let (cmd_tx, mut cmd_rx) = mpsc::channel::<Command>(32);
    let (ready_tx, ready_rx) = oneshot::channel::<Result<Handshake, AcpError>>();

    let notif_tx = ev_tx.clone();
    let auto_approve = opts.auto_approve;

    tokio::spawn(async move {
        let _keepalive = keepalive;
        let run_tx = ev_tx.clone();
        let result = Client
            .v1()
            .name("tantra-tui")
            .on_receive_notification(
                async move |n: SessionNotification, _cx: ConnectionTo<Agent>| {
                    if let Some(ev) = map_notification(n) {
                        let _ = notif_tx.send(ev).await;
                    }
                    Ok(())
                },
                agent_client_protocol::on_receive_notification!(),
            )
            .on_receive_request(
                async move |_req: RequestPermissionRequest,
                            responder: Responder<RequestPermissionResponse>,
                            _cx: ConnectionTo<Agent>| {
                    let id = if auto_approve {
                        "allow_once"
                    } else {
                        "reject_once"
                    };
                    responder.respond(RequestPermissionResponse::new(
                        RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(
                            PermissionOptionId::new(id),
                        )),
                    ))
                },
                agent_client_protocol::on_receive_request!(),
            )
            .connect_with(
                ByteStreams::new(writer.compat_write(), reader.compat()),
                async move |cx: ConnectionTo<Agent>| {
                    let (hs, sid) = match handshake(&cx, &opts).await {
                        Ok(v) => v,
                        Err(e) => {
                            let _ = ready_tx.send(Err(AcpError::from(e)));
                            return Ok(());
                        }
                    };
                    if ready_tx.send(Ok(hs)).is_err() {
                        return Ok(());
                    }
                    while let Some(cmd) = cmd_rx.recv().await {
                        match cmd {
                            Command::Prompt(text) => {
                                let cx2 = cx.clone();
                                let sid = sid.clone();
                                let tx = run_tx.clone();
                                cx.spawn(async move {
                                    let ev = match cx2
                                        .send_request(PromptRequest::new(
                                            sid,
                                            vec![ContentBlock::from(text)],
                                        ))
                                        .block_task()
                                        .await
                                    {
                                        Ok(r) => ClientEvent::TurnEnded(r.stop_reason),
                                        Err(e) => ClientEvent::Error(e.to_string()),
                                    };
                                    let _ = tx.send(ev).await;
                                    Ok(())
                                })?;
                            }
                            Command::Cancel => {
                                cx.send_notification(CancelNotification::new(sid.clone()))?;
                            }
                            Command::SetModel(model, reply) => {
                                let params = json!({ "sessionId": &*sid.0, "model": model });
                                let res = match UntypedMessage::new("_tantra/set_model", params) {
                                    Ok(msg) => cx
                                        .send_request(msg)
                                        .block_task()
                                        .await
                                        .map_err(AcpError::from)
                                        .and_then(|v: Value| {
                                            v.get("model")
                                                .and_then(Value::as_str)
                                                .map(str::to_string)
                                                .ok_or_else(|| {
                                                    AcpError::Protocol(
                                                        "set_model: missing model".into(),
                                                    )
                                                })
                                        }),
                                    Err(e) => Err(AcpError::from(e)),
                                };
                                let _ = reply.send(res);
                            }
                        }
                    }
                    Ok(())
                },
            )
            .await;
        if let Err(e) = result {
            tracing::debug!("acp client connection ended: {e}");
        }
    });

    let hs = ready_rx.await.map_err(|_| AcpError::Closed)??;
    Ok((
        AcpClient {
            tx: cmd_tx,
            models: hs.models,
            default_model: hs.default_model,
            model: hs.model,
        },
        ev_rx,
    ))
}

/// In-process client: the agent is served on one end of a bounded duplex pipe.
pub async fn connect_embedded(
    host: Arc<AgentHost>,
    opts: ClientOptions,
) -> Result<(AcpClient, mpsc::Receiver<ClientEvent>), AcpError> {
    let (a_io, c_io) = tokio::io::duplex(256 * 1024);
    let (a_r, a_w) = tokio::io::split(a_io);
    let (c_r, c_w) = tokio::io::split(c_io);
    tokio::spawn(async move {
        if let Err(e) = serve(host, a_r, a_w).await {
            tracing::debug!("embedded acp agent ended: {e}");
        }
    });
    AcpClient::connect(c_r, c_w, opts).await
}

/// Spawn an ACP agent child process (e.g. `tantra-acp`) and talk over its pipes.
pub async fn connect_stdio(
    command: &str,
    args: &[String],
    opts: ClientOptions,
) -> Result<(AcpClient, mpsc::Receiver<ClientEvent>), AcpError> {
    let mut child = tokio::process::Command::new(command)
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| AcpError::Protocol("child stdin unavailable".into()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AcpError::Protocol("child stdout unavailable".into()))?;
    connect_inner(stdout, stdin, opts, Some(Box::new(child))).await
}
