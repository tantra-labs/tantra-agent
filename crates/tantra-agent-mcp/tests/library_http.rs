use std::sync::{Arc, Mutex};

use serde_json::json;
use tantra_agent_core::library::{
    DescribeFormat, IndicatorLibraryPort, LibraryDescribeRequest, LibraryPortErrorKind,
    LibrarySearchRequest,
};
use tantra_agent_mcp::RuntimeLibraryClient;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Serve one canned response; returns (base_url, captured request head).
async fn serve(status: &str, body: String) -> (String, Arc<Mutex<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let captured = Arc::new(Mutex::new(String::new()));
    let cap = captured.clone();
    let status = status.to_string();
    tokio::spawn(async move {
        if let Ok((mut sock, _)) = listener.accept().await {
            let mut buf = vec![0u8; 8192];
            let n = sock.read(&mut buf).await.unwrap_or(0);
            *cap.lock().unwrap() = String::from_utf8_lossy(&buf[..n]).to_string();
            let resp = format!(
                "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = sock.write_all(resp.as_bytes()).await;
            let _ = sock.shutdown().await;
        }
    });
    (format!("http://{addr}"), captured)
}

#[tokio::test]
async fn unwraps_envelope_and_forwards_bearer() {
    let (url, cap) = serve(
        "200 OK",
        json!({"api_version":"v2","data":{"items":[1]}}).to_string(),
    )
    .await;
    let c = RuntimeLibraryClient::new(url, Some("sekret".into())).unwrap();
    let v = c
        .search(LibrarySearchRequest {
            query: Some("rsi".into()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(v, json!({"items":[1]}));
    let head = cap.lock().unwrap().to_lowercase();
    assert!(head.contains("authorization: bearer sekret"), "{head}");
    assert!(head.contains("query=rsi"), "{head}");
}

#[tokio::test]
async fn no_bearer_when_unconfigured_and_id_is_encoded() {
    let (url, cap) = serve("200 OK", json!({"id":"a"}).to_string()).await;
    let c = RuntimeLibraryClient::new(url, None).unwrap();
    c.describe(LibraryDescribeRequest {
        id: "a/b c".into(),
        format: DescribeFormat::Json,
    })
    .await
    .unwrap();
    let head = cap.lock().unwrap().to_lowercase();
    assert!(!head.contains("authorization"), "{head}");
    assert!(head.contains("/entries/a%2fb%20c?format=json"), "{head}");
}

#[tokio::test]
async fn status_mapping() {
    for (st, kind) in [
        ("503 Service Unavailable", LibraryPortErrorKind::Unavailable),
        ("404 Not Found", LibraryPortErrorKind::NotFound),
        ("429 Too Many Requests", LibraryPortErrorKind::Busy),
        ("504 Gateway Timeout", LibraryPortErrorKind::Timeout),
        (
            "422 Unprocessable Entity",
            LibraryPortErrorKind::InvalidRequest,
        ),
        ("502 Bad Gateway", LibraryPortErrorKind::Upstream),
        ("401 Unauthorized", LibraryPortErrorKind::Upstream),
    ] {
        let (url, _) = serve(st, "{}".into()).await;
        let c = RuntimeLibraryClient::new(url, None).unwrap();
        let e = c.search(LibrarySearchRequest::default()).await.unwrap_err();
        assert_eq!(e.kind, kind, "{st}");
    }
}

#[tokio::test]
async fn oversized_body_is_rejected() {
    let (url, _) = serve("200 OK", json!({"data":"x".repeat(4096)}).to_string()).await;
    let c = RuntimeLibraryClient::new(url, None)
        .unwrap()
        .with_max_response_bytes(1024);
    let e = c.search(LibrarySearchRequest::default()).await.unwrap_err();
    assert_eq!(e.kind, LibraryPortErrorKind::Upstream);
}
