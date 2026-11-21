//! ACP agent side: serves `run_turn` over the Agent Client Protocol.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::v1::schema::*;
use agent_client_protocol::v1::{ConnectionTo, Responder};
use agent_client_protocol::{Agent, ByteStreams, Client, Error, UntypedMessage};
use serde_json::{Value, json};
use tantra_agent_chat::{
    CancellationToken, ChatError, PermissionDecision, SessionEvent, ToolOutcome, ToolSet,
    TurnConfig, TurnId, TurnOutcome, run_turn,
};
use tantra_agent_core::{PersistedMessage, SessionStore};
use tokio::sync::{Mutex, mpsc};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

use crate::catalog::ModelCatalog;
use crate::error::AcpError;
use crate::meta::{read_tantra, tantra_meta};

/// Everything the agent needs from its host (composition root).
pub struct AgentHost {
    pub system_prompt: String,
    pub tools: Arc<ToolSet>,
    pub store: Arc<dyn SessionStore>,
    pub catalog: Arc<dyn ModelCatalog>,
    pub turn_config: TurnConfig,
    pub max_sessions: usize,
}

struct SessionState {
    model: String,
    history: Vec<PersistedMessage>,
    grants: HashSet<String>,
    cancel: Option<CancellationToken>,
    turn: u64,
    busy: bool,
}

type Sessions = Arc<Mutex<HashMap<String, SessionState>>>;

struct Prep {
    model: String,
    history: Vec<PersistedMessage>,
    grants: HashSet<String>,
    turn: u64,
    token: CancellationToken,
}

fn invalid(msg: impl Into<String>) -> Error {
    Error::new(ErrorCode::InvalidParams, msg.into())
}

fn models_value(host: &AgentHost) -> Value {
    json!({ "models": host.catalog.models(), "defaultModel": host.catalog.default_model() })
}

/// Serve one ACP connection over the given byte streams until it closes.
pub async fn serve(
    host: Arc<AgentHost>,
    reader: impl tokio::io::AsyncRead + Send + Unpin + 'static,
    writer: impl tokio::io::AsyncWrite + Send + Unpin + 'static,
) -> Result<(), AcpError> {
    let sessions: Sessions = Arc::new(Mutex::new(HashMap::new()));

    let h_init = host.clone();
    let h_new = host.clone();
    let s_new = sessions.clone();
    let h_prompt = host.clone();
    let s_prompt = sessions.clone();
    let s_cancel = sessions.clone();
    let h_any = host.clone();
    let s_any = sessions.clone();

    let agent = Agent
        .v1()
        .name("tantra")
        .on_receive_request(
            async move |_req: InitializeRequest,
                        responder: Responder<InitializeResponse>,
                        _cx: ConnectionTo<Client>| {
                responder.respond(
                    InitializeResponse::new(ProtocolVersion::V1)
                        .agent_capabilities(AgentCapabilities::new().load_session(false))
                        .agent_info(
                            Implementation::new("tantra", env!("CARGO_PKG_VERSION"))
                                .title("Tantra".to_string()),
                        )
                        .meta(tantra_meta(models_value(&h_init))),
                )
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |req: NewSessionRequest,
                        responder: Responder<NewSessionResponse>,
                        _cx: ConnectionTo<Client>| {
                match new_session(&h_new, &s_new, &req).await {
                    Ok(resp) => responder.respond(resp),
                    Err(e) => responder.respond_with_error(e),
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |req: PromptRequest,
                        responder: Responder<PromptResponse>,
                        cx: ConnectionTo<Client>| {
                let prep = match prepare_prompt(&s_prompt, &req).await {
                    Ok(p) => p,
                    Err(e) => return responder.respond_with_error(e),
                };
                let host = h_prompt.clone();
                let sessions = s_prompt.clone();
                let cx2 = cx.clone();
                cx.spawn(async move {
                    run_prompt(host, sessions, req, prep, responder, cx2).await;
                    Ok(())
                })
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_notification(
            async move |n: CancelNotification, _cx: ConnectionTo<Client>| {
                let guard = s_cancel.lock().await;
                if let Some(token) = guard.get(&*n.session_id.0).and_then(|s| s.cancel.as_ref()) {
                    token.cancel();
                }
                Ok(())
            },
            agent_client_protocol::on_receive_notification!(),
        )
        .on_receive_request(
            async move |req: UntypedMessage,
                        responder: Responder<Value>,
                        _cx: ConnectionTo<Client>| {
                match ext_method(&h_any, &s_any, req.method(), req.params()).await {
                    Ok(v) => responder.respond(v),
                    Err(e) => responder.respond_with_error(e),
                }
            },
            agent_client_protocol::on_receive_request!(),
        );

    agent
        .connect_to(ByteStreams::new(writer.compat_write(), reader.compat()))
        .await?;
    Ok(())
}

async fn new_session(
    host: &AgentHost,
    sessions: &Sessions,
    req: &NewSessionRequest,
) -> Result<NewSessionResponse, Error> {
    let model = read_tantra(&req.meta)
        .and_then(|v| v.get("model"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| host.catalog.default_model());
    host.catalog
        .provider(&model)
        .map_err(|_| invalid(format!("unknown model: {model}")))?;
    let mut guard = sessions.lock().await;
    if guard.len() >= host.max_sessions {
        return Err(invalid(format!(
            "session limit reached ({})",
            host.max_sessions
        )));
    }
    let id = format!("tantra-{}", uuid::Uuid::new_v4());
    guard.insert(
        id.clone(),
        SessionState {
            model: model.clone(),
            history: Vec::new(),
            grants: HashSet::new(),
            cancel: None,
            turn: 0,
            busy: false,
        },
    );
    Ok(NewSessionResponse::new(SessionId::new(id)).meta(tantra_meta(json!({ "model": model }))))
}

async fn prepare_prompt(sessions: &Sessions, req: &PromptRequest) -> Result<Prep, Error> {
    let mut guard = sessions.lock().await;
    let st = guard
        .get_mut(&*req.session_id.0)
        .ok_or_else(|| invalid("unknown session"))?;
    if st.busy {
        return Err(invalid("a prompt is already running"));
    }
    st.busy = true;
    st.turn += 1;
    let token = CancellationToken::new();
    st.cancel = Some(token.clone());
    Ok(Prep {
        model: st.model.clone(),
        history: st.history.clone(),
        grants: st.grants.clone(),
        turn: st.turn,
        token,
    })
}

async fn clear_busy(sessions: &Sessions, sid: &str) {
    if let Some(st) = sessions.lock().await.get_mut(sid) {
        st.busy = false;
        st.cancel = None;
    }
}

fn prompt_text(req: &PromptRequest) -> String {
    req.prompt
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text(t) => Some(t.text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

async fn run_prompt(
    host: Arc<AgentHost>,
    sessions: Sessions,
    req: PromptRequest,
    prep: Prep,
    responder: Responder<PromptResponse>,
    cx: ConnectionTo<Client>,
) {
    let sid = req.session_id.clone();
    let sid_str = sid.0.to_string();
    let text = prompt_text(&req);

    let provider = match host.catalog.provider(&prep.model) {
        Ok(p) => p,
        Err(e) => {
            clear_busy(&sessions, &sid_str).await;
            let _ =
                responder.respond_with_error(Error::new(ErrorCode::InternalError, e.to_string()));
            return;
        }
    };

    let (tx, mut rx) = mpsc::channel::<SessionEvent>(256);
    let turn_fut = run_turn(
        None,
        TurnId(prep.turn),
        &host.system_prompt,
        prep.history,
        &text,
        Vec::new(),
        &host.tools,
        provider,
        host.store.clone(),
        tx,
        &host.turn_config,
        prep.token,
        prep.grants,
    );
    let pump = async {
        while let Some(ev) = rx.recv().await {
            if let Err(e) = handle_event(&cx, &sid, ev).await {
                tracing::warn!("acp event delivery failed: {e}");
            }
        }
    };
    let (result, ()) = tokio::join!(turn_fut, pump);

    let response = match result {
        Ok((outcome, history, grants)) => {
            if let Some(st) = sessions.lock().await.get_mut(&sid_str) {
                st.history = history;
                st.grants = grants;
            }
            match outcome {
                TurnOutcome::Completed => Ok(StopReason::EndTurn),
                TurnOutcome::MaxTokens => Ok(StopReason::MaxTokens),
                TurnOutcome::Refused { .. } => Ok(StopReason::Refusal),
                TurnOutcome::RoundLimit { .. } => Ok(StopReason::MaxTurnRequests),
                TurnOutcome::Stopped | TurnOutcome::Cancelled => Ok(StopReason::Cancelled),
                TurnOutcome::Failed { error } => Err(error.message),
                #[allow(unreachable_patterns)]
                _ => Ok(StopReason::EndTurn),
            }
        }
        Err(e) => Err(ChatError::to_string(&e)),
    };
    clear_busy(&sessions, &sid_str).await;
    let _ = match response {
        Ok(stop) => responder.respond(PromptResponse::new(stop)),
        Err(msg) => responder.respond_with_error(Error::new(ErrorCode::InternalError, msg)),
    };
}

fn text_chunk(text: String) -> ContentChunk {
    ContentChunk::new(ContentBlock::from(text))
}

async fn handle_event(
    cx: &ConnectionTo<Client>,
    sid: &SessionId,
    ev: SessionEvent,
) -> Result<(), Error> {
    let update = match ev {
        SessionEvent::TextDelta { text, .. } => SessionUpdate::AgentMessageChunk(text_chunk(text)),
        SessionEvent::ThinkingDelta { text, .. } => {
            SessionUpdate::AgentThoughtChunk(text_chunk(text))
        }
        SessionEvent::ToolStarted {
            call_id,
            name,
            summary,
            ..
        } => SessionUpdate::ToolCall(
            ToolCall::new(ToolCallId::new(call_id.0), summary)
                .kind(ToolKind::Other)
                .status(ToolCallStatus::InProgress)
                .meta(tantra_meta(json!({ "tool": name }))),
        ),
        SessionEvent::ToolFinished {
            call_id,
            name,
            outcome,
            elapsed_ms,
            blocks,
            ..
        } => {
            let (status, detail) = match outcome {
                ToolOutcome::Success => (ToolCallStatus::Completed, "ok".to_string()),
                ToolOutcome::Error(m) | ToolOutcome::Denied(m) => (ToolCallStatus::Failed, m),
                #[allow(unreachable_patterns)]
                _ => (ToolCallStatus::Failed, "failed".to_string()),
            };
            let blocks_v = serde_json::to_value(&blocks).unwrap_or_else(|_| json!([]));
            SessionUpdate::ToolCallUpdate(
                ToolCallUpdate::new(
                    ToolCallId::new(call_id.0),
                    ToolCallUpdateFields::new()
                        .status(status)
                        .content(vec![ToolCallContent::from(detail)]),
                )
                .meta(tantra_meta(
                    json!({ "blocks": blocks_v, "elapsedMs": elapsed_ms, "tool": name }),
                )),
            )
        }
        SessionEvent::PermissionRequested(mut prompt) => {
            let decision =
                ask_permission(cx, sid, &prompt.tool, &prompt.summary, &prompt.input).await;
            if let Some(responder) = prompt.responder.take() {
                if let Err(e) = responder.respond(decision) {
                    tracing::warn!("permission responder failed: {e}");
                }
            }
            return Ok(());
        }
        _ => return Ok(()),
    };
    cx.send_notification(SessionNotification::new(sid.clone(), update))
}

async fn ask_permission(
    cx: &ConnectionTo<Client>,
    sid: &SessionId,
    tool: &str,
    summary: &str,
    input: &Value,
) -> PermissionDecision {
    let deny = || PermissionDecision::Deny {
        feedback: Some("rejected by client".to_string()),
    };
    let req = RequestPermissionRequest::new(
        sid.clone(),
        ToolCallUpdate::new(
            ToolCallId::new(format!("perm-{tool}")),
            ToolCallUpdateFields::new()
                .title(summary.to_string())
                .raw_input(input.clone()),
        ),
        vec![
            PermissionOption::new(
                PermissionOptionId::new("allow_once"),
                "Allow once",
                PermissionOptionKind::AllowOnce,
            ),
            PermissionOption::new(
                PermissionOptionId::new("allow_always"),
                "Allow always",
                PermissionOptionKind::AllowAlways,
            ),
            PermissionOption::new(
                PermissionOptionId::new("reject_once"),
                "Reject",
                PermissionOptionKind::RejectOnce,
            ),
        ],
    );
    match cx.send_request(req).block_task().await {
        Ok(resp) => match resp.outcome {
            RequestPermissionOutcome::Selected(s) => match &*s.option_id.0 {
                "allow_once" => PermissionDecision::AllowOnce,
                "allow_always" => PermissionDecision::AllowForSession,
                _ => deny(),
            },
            _ => deny(),
        },
        Err(_) => deny(),
    }
}

async fn ext_method(
    host: &AgentHost,
    sessions: &Sessions,
    method: &str,
    params: &Value,
) -> Result<Value, Error> {
    match method {
        "_tantra/models" => Ok(models_value(host)),
        "_tantra/set_model" => {
            let sid = params
                .get("sessionId")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid("missing sessionId"))?;
            let model = params
                .get("model")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid("missing model"))?;
            host.catalog
                .provider(model)
                .map_err(|_| invalid(format!("unknown model: {model}")))?;
            let mut guard = sessions.lock().await;
            let st = guard
                .get_mut(sid)
                .ok_or_else(|| invalid("unknown session"))?;
            st.model = model.to_string();
            Ok(json!({ "model": model }))
        }
        _ => Err(Error::method_not_found()),
    }
}
