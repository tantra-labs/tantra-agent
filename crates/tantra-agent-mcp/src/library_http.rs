//! HTTP implementation of [`IndicatorLibraryPort`] against the runtime's canonical
//! `/api/v2/studies/*` routes (the runtime supervises the Python library process).
//!
//! Hardening: explicit timeouts, TLS verification left on, bounded response bodies, and the
//! bearer token (if any) is never logged or included in errors.

use std::time::Duration;

use serde::Serialize;
use serde_json::Value;
use tantra_agent_core::library::{
    DescribeFormat, IndicatorComputeRequest, IndicatorLibraryPort, LibraryDescribeRequest,
    LibraryPortError, LibrarySearchRequest, SignalStudyRequest,
};
use tantra_agent_core::strategy_loop::{
    StrategyHedgeRequest, StrategyLoopEvalRequest, StrategyLoopPort, StrategyLoopPortError,
    StrategyLoopSweepRequest,
};

/// Routes this client calls as `(METHOD, path)`, relative to `/api/v2`, in OpenAPI `{param}`
/// spelling. Checked against the runtime's `openapi.v2.json` by `tests/runtime_contract.rs`.
pub const RUNTIME_ROUTES: &[(&str, &str)] = &[
    ("GET", "/studies/library/entries"),
    ("GET", "/studies/library/entries/{id}"),
    ("POST", "/studies/indicators/compute"),
    ("POST", "/studies/signal-study"),
    ("POST", "/studies/strategy-loop/evaluate"),
    ("POST", "/studies/strategy-loop/sweep"),
    ("POST", "/studies/strategy-loop/recommend-hedge"),
];

/// Maximum accepted response body size (16 MiB).
pub const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;

/// HTTP client for the runtime's library routes.
pub struct RuntimeLibraryClient {
    http: reqwest::Client,
    base_url: String,
    token: Option<String>,
    max_response_bytes: usize,
}

impl std::fmt::Debug for RuntimeLibraryClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuntimeLibraryClient")
            .field("base_url", &self.base_url)
            .field("token", &self.token.as_ref().map(|_| "[REDACTED]"))
            .finish()
    }
}

impl RuntimeLibraryClient {
    /// Build a client for `base_url` (e.g. `http://127.0.0.1:8080`) with an optional bearer token.
    pub fn new(
        base_url: impl Into<String>,
        token: Option<String>,
    ) -> Result<Self, LibraryPortError> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .connect_timeout(Duration::from_secs(5))
            .build()
            .map_err(|e| LibraryPortError::unavailable(format!("http client init failed: {e}")))?;
        Ok(Self {
            http,
            base_url: base_url.into().trim_end_matches('/').to_string(),
            token: token.filter(|t| !t.is_empty()),
            max_response_bytes: MAX_RESPONSE_BYTES,
        })
    }

    /// Override the response size cap (primarily for tests).
    pub fn with_max_response_bytes(mut self, max: usize) -> Self {
        self.max_response_bytes = max;
        self
    }

    /// Read `TANTRA_RUNTIME_URL` and optional `TANTRA_RUNTIME_TOKEN`; `None` when URL is unset.
    pub fn from_env() -> Option<Self> {
        let url = std::env::var("TANTRA_RUNTIME_URL").ok()?;
        if url.trim().is_empty() {
            return None;
        }
        let token = std::env::var("TANTRA_RUNTIME_TOKEN").ok();
        match Self::new(url.trim(), token) {
            Ok(c) => Some(c),
            Err(e) => {
                tracing::warn!(error = %e, "runtime library client disabled");
                None
            }
        }
    }

    /// Build from the `runtime` block of a loaded `tantra.jsonc`; `None` when absent or invalid.
    pub fn from_config(loaded: &tantra_agent_llm::LoadedConfig) -> Option<Self> {
        let env = |k: &str| std::env::var(k).ok();
        let endpoint = match tantra_agent_llm::resolve_runtime(loaded, &env) {
            Ok(Some(e)) => e,
            Ok(None) => return None,
            Err(e) => {
                tracing::warn!(error = %e, "tantra.jsonc runtime block unusable");
                return None;
            }
        };
        match Self::new(endpoint.url, endpoint.token) {
            Ok(c) => Some(c),
            Err(e) => {
                tracing::warn!(error = %e, "runtime library client disabled");
                None
            }
        }
    }

    /// Discover the runtime: `tantra.jsonc` `runtime` block first, then the deprecated
    /// `TANTRA_RUNTIME_URL` / `TANTRA_RUNTIME_TOKEN` environment variables.
    pub fn discover() -> Option<Self> {
        if let Ok(loaded) = tantra_agent_llm::load_config(None)
            && let Some(c) = Self::from_config(&loaded)
        {
            return Some(c);
        }
        let c = Self::from_env()?;
        tracing::warn!(
            "TANTRA_RUNTIME_URL/TANTRA_RUNTIME_TOKEN are deprecated; use the `runtime` block in tantra.jsonc"
        );
        Some(c)
    }

    fn authed(&self, rb: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.token {
            Some(t) => rb.bearer_auth(t),
            None => rb,
        }
    }

    async fn send(&self, rb: reqwest::RequestBuilder) -> Result<Value, LibraryPortError> {
        let mut resp = self.authed(rb).send().await.map_err(|e| {
            if e.is_timeout() {
                LibraryPortError::timeout("runtime request timed out")
            } else {
                LibraryPortError::unavailable("runtime unreachable")
            }
        })?;
        let status = resp.status();
        if resp
            .content_length()
            .is_some_and(|l| l > self.max_response_bytes as u64)
        {
            return Err(LibraryPortError::upstream("response exceeds size cap"));
        }
        let mut body: Vec<u8> = Vec::new();
        while let Some(chunk) = resp
            .chunk()
            .await
            .map_err(|_| LibraryPortError::upstream("failed reading response body"))?
        {
            if body.len() + chunk.len() > self.max_response_bytes {
                return Err(LibraryPortError::upstream("response exceeds size cap"));
            }
            body.extend_from_slice(&chunk);
        }
        let parsed: Option<Value> = serde_json::from_slice(&body).ok();
        if !status.is_success() {
            let detail = parsed
                .as_ref()
                .and_then(|v| v.pointer("/error/message").and_then(Value::as_str))
                .unwrap_or("");
            return Err(map_status(status.as_u16(), detail));
        }
        let value =
            parsed.ok_or_else(|| LibraryPortError::upstream("response is not valid JSON"))?;
        Ok(unwrap_envelope(value))
    }

    async fn post<T: Serialize>(&self, path: &str, body: &T) -> Result<Value, LibraryPortError> {
        let url = format!("{}{}", self.base_url, path);
        self.send(self.http.post(url).json(body)).await
    }
}

fn map_status(code: u16, detail: &str) -> LibraryPortError {
    let msg = |d: &str| {
        if detail.is_empty() {
            d.to_string()
        } else {
            format!("{d}: {detail}")
        }
    };
    match code {
        503 => LibraryPortError::unavailable(msg("library unavailable")),
        429 => LibraryPortError::busy(msg("library busy")),
        504 => LibraryPortError::timeout(msg("library timeout")),
        404 => LibraryPortError::not_found(msg("not found")),
        400 | 413 | 422 => LibraryPortError::invalid_request(msg("invalid request")),
        401 | 403 => LibraryPortError::upstream("unauthorized"),
        c => LibraryPortError::upstream(msg(&format!("runtime returned status {c}"))),
    }
}

/// Unwrap the v2 envelope (`{"api_version":"v2","data":...}`) if present.
fn unwrap_envelope(v: Value) -> Value {
    match v {
        Value::Object(mut m) if m.contains_key("api_version") && m.contains_key("data") => {
            m.remove("data").unwrap_or(Value::Null)
        }
        other => other,
    }
}

/// Percent-encode a single path segment (RFC 3986 unreserved characters pass through).
fn encode_segment(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[async_trait::async_trait]
impl IndicatorLibraryPort for RuntimeLibraryClient {
    async fn search(&self, request: LibrarySearchRequest) -> Result<Value, LibraryPortError> {
        let mut q: Vec<(&str, String)> = Vec::new();
        if let Some(v) = request.query {
            q.push(("query", v));
        }
        if let Some(v) = request.kind {
            q.push(("kind", v));
        }
        if let Some(v) = request.family {
            q.push(("family", v));
        }
        if let Some(v) = request.limit {
            q.push(("limit", v.to_string()));
        }
        let url = format!("{}/api/v2/studies/library/entries", self.base_url);
        self.send(self.http.get(url).query(&q)).await
    }

    async fn describe(&self, request: LibraryDescribeRequest) -> Result<Value, LibraryPortError> {
        let format = match request.format {
            DescribeFormat::Markdown => "markdown",
            DescribeFormat::Json => "json",
        };
        let url = format!(
            "{}/api/v2/studies/library/entries/{}",
            self.base_url,
            encode_segment(&request.id)
        );
        self.send(self.http.get(url).query(&[("format", format)]))
            .await
    }

    async fn compute(&self, request: IndicatorComputeRequest) -> Result<Value, LibraryPortError> {
        self.post("/api/v2/studies/indicators/compute", &request)
            .await
    }

    async fn signal_study(&self, request: SignalStudyRequest) -> Result<Value, LibraryPortError> {
        self.post("/api/v2/studies/signal-study", &request).await
    }
}

#[async_trait::async_trait]
impl StrategyLoopPort for RuntimeLibraryClient {
    async fn evaluate(&self, req: StrategyLoopEvalRequest) -> Result<Value, StrategyLoopPortError> {
        Ok(self
            .post("/api/v2/studies/strategy-loop/evaluate", &req)
            .await?)
    }

    async fn sweep(&self, req: StrategyLoopSweepRequest) -> Result<Value, StrategyLoopPortError> {
        Ok(self
            .post("/api/v2/studies/strategy-loop/sweep", &req)
            .await?)
    }

    async fn recommend_hedge(
        &self,
        req: StrategyHedgeRequest,
    ) -> Result<Value, StrategyLoopPortError> {
        Ok(self
            .post("/api/v2/studies/strategy-loop/recommend-hedge", &req)
            .await?)
    }
}
