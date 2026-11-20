use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use tantra_agent_core::library::{
    IndicatorComputeRequest, IndicatorLibraryPort, LibraryDescribeRequest, LibraryPortError,
    LibrarySearchRequest, SignalStudyRequest,
};
use tantra_agent_mcp::{McpError, McpServer, list_tool_definitions};

#[derive(Default)]
struct Fake {
    calls: Mutex<Vec<String>>,
}

impl Fake {
    fn rec(&self, s: &str) {
        self.calls.lock().unwrap().push(s.to_string());
    }
}

#[async_trait::async_trait]
impl IndicatorLibraryPort for Fake {
    async fn search(&self, r: LibrarySearchRequest) -> Result<Value, LibraryPortError> {
        self.rec("search");
        if r.kind.as_deref() == Some("indicator") {
            return Ok(json!({"results":[{"id":"ind.predictive-ranges","computable":true}]}));
        }
        Ok(json!({"items":[]}))
    }
    async fn describe(&self, r: LibraryDescribeRequest) -> Result<Value, LibraryPortError> {
        self.rec("describe");
        if r.id == "ind.predictive-ranges" {
            return Ok(json!({
                "id":"ind.predictive-ranges","kind":"indicator","title":"Predictive Ranges",
                "aliases":[],"params":[]
            }));
        }
        Ok(json!({"id": r.id}))
    }
    async fn compute(&self, _r: IndicatorComputeRequest) -> Result<Value, LibraryPortError> {
        self.rec("compute");
        Ok(json!({"rows":1}))
    }
    async fn signal_study(&self, _r: SignalStudyRequest) -> Result<Value, LibraryPortError> {
        self.rec("signal_study");
        Ok(json!({"events":2}))
    }
}

fn server() -> (McpServer, Arc<Fake>) {
    let fake = Arc::new(Fake::default());
    (McpServer::new().with_library_port(fake.clone()), fake)
}

#[tokio::test]
async fn four_tools_round_trip() {
    let (s, fake) = server();
    let r = s
        .call_tool("tantra_library_search", json!({"query":"rsi","limit":5}))
        .await
        .unwrap();
    assert_eq!(r["data"]["items"], json!([]));
    s.call_tool("tantra_library_describe", json!({"id":"x","format":"json"}))
        .await
        .unwrap();
    let r = s
        .call_tool(
            "tantra_indicator_compute",
            json!({"id":"x","params":{},"instrument":"A","bars":[{"c":1}],"tail":10}),
        )
        .await
        .unwrap();
    assert_eq!(r["data"]["rows"], 1);
    let r = s
        .call_tool(
            "tantra_signal_study",
            json!({"id":"x","params":{},"instrument":"A","bars_file":"main",
                   "event_column":"e","horizons":[1,5]}),
        )
        .await
        .unwrap();
    assert_eq!(r["data"]["events"], 2);
    assert_eq!(
        *fake.calls.lock().unwrap(),
        vec!["search", "describe", "compute", "signal_study"]
    );
}

#[tokio::test]
async fn invalid_args_never_reach_port() {
    let (s, fake) = server();
    let base = json!({"id":"x","params":{},"instrument":"A","bars_file":"main"});
    let mut tail = base.clone();
    tail["tail"] = json!(5000);
    assert!(matches!(
        s.call_tool("tantra_indicator_compute", tail).await,
        Err(McpError::InvalidArguments(_))
    ));
    let mut hz = base.clone();
    hz["event_column"] = json!("e");
    hz["horizons"] = json!((1..=11).collect::<Vec<u32>>());
    assert!(matches!(
        s.call_tool("tantra_signal_study", hz).await,
        Err(McpError::InvalidArguments(_))
    ));
    let mut unk = base.clone();
    unk["bogus"] = json!(1);
    assert!(matches!(
        s.call_tool("tantra_indicator_compute", unk).await,
        Err(McpError::InvalidArguments(_))
    ));
    assert!(matches!(
        s.call_tool("tantra_library_search", json!({"limit":201}))
            .await,
        Err(McpError::InvalidArguments(_))
    ));
    assert!(fake.calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn no_port_is_explicit_error() {
    let s = McpServer::new();
    let err = s
        .call_tool("tantra_library_search", json!({}))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("TANTRA_RUNTIME_URL"), "{err}");
}

#[tokio::test]
async fn check_spec_uses_library_catalog_when_configured() {
    let spec = json!({"strategy_manifest":{
        "name":"S","indicators":[{"name":"predictive-ranges","params":{}}]}});
    let plain = McpServer::new();
    assert!(plain.call_tool("check_spec", spec.clone()).await.is_err());
    let (s, _) = server();
    let r = s.call_tool("check_spec", spec).await.unwrap();
    assert_eq!(r["data"]["status"], "valid");
}

#[test]
fn definitions_include_library_tools() {
    let defs = list_tool_definitions();
    for name in [
        "tantra_library_search",
        "tantra_library_describe",
        "tantra_indicator_compute",
        "tantra_signal_study",
    ] {
        let d = defs.iter().find(|d| d["name"] == name).expect(name);
        assert_eq!(d["inputSchema"]["additionalProperties"], false, "{name}");
    }
}
