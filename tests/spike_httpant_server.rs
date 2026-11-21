//! Spike, widened (DESIGN s4b HTTP row): rmcp's `StreamableHttpService` behind a RUNNING HTTPAnt `Server` (real socket, HTTPAnt's
//! router, limits and shutdown), called by HTTPAnt's own `Client`. The first spike used a bare `tower::service_fn`.

use std::{convert::Infallible, sync::Arc};

use httpant::{Body, Client, Request, Response, Server};
use rmcp::{
    handler::server::wrapper::Parameters,
    tool, tool_router,
    transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    },
};
use tower::Service;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct AddParams {
    a: i32,
    b: i32,
}

#[derive(Clone)]
struct Calc;

#[tool_router(server_handler)]
impl Calc {
    #[tool(description = "Add two numbers")]
    fn add(&self, Parameters(AddParams { a, b }): Parameters<AddParams>) -> String {
        (a + b).to_string()
    }
}

#[tokio::test]
async fn rmcp_streamable_http_serves_from_a_running_httpant_server() {
    let rmcp_svc = StreamableHttpService::new(
        || Ok(Calc),
        Arc::new(LocalSessionManager::default()),
        // Host/origin checks are rmcp's; a loopback test names the host it connects to.
        StreamableHttpServerConfig::default(),
    );
    let mount = move |req: Request| {
        let mut inner = rmcp_svc.clone();
        async move {
            let resp = Service::<Request>::call(&mut inner, req)
                .await
                .unwrap_or_else(|e: Infallible| match e {});
            resp.map(Body::new) as Response
        }
    };
    let server = Server::new()
        .post("/mcp", mount)
        .bind("127.0.0.1:0")
        .await
        .expect("bind");
    let url = format!("http://{}/mcp", server.addr());
    let init = serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"spike","version":"0"}}});
    let resp = Client::new()
        .post(url)
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .body(init.to_string())
        .send()
        .await
        .expect("send");
    let status = resp.status();
    let text = resp.text().await.expect("body");
    println!("status={status}\n{text}");
    assert!(status.is_success(), "{status}: {text}");
    assert!(
        text.contains("protocolVersion"),
        "no InitializeResult: {text}"
    );
}

/// 2026-07-28 spec: an unsupported protocol version over HTTP MUST be answered `400 Bad Request` with `UnsupportedProtocolVersionError`.
/// The `MCP-Protocol-Version` header must accompany the `_meta` version (without it rmcp answers 400 with -32020, a different error).
#[tokio::test]
async fn unsupported_version_over_http_status_is_reported() {
    let rmcp_svc = StreamableHttpService::new(
        || Ok(Calc),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );
    let mount = move |req: Request| {
        let mut inner = rmcp_svc.clone();
        async move {
            let resp = Service::<Request>::call(&mut inner, req)
                .await
                .unwrap_or_else(|e: Infallible| match e {});
            resp.map(Body::new) as Response
        }
    };
    let server = Server::new()
        .post("/mcp", mount)
        .bind("127.0.0.1:0")
        .await
        .expect("bind");
    let bad = serde_json::json!({"jsonrpc":"2.0","id":4,"method":"tools/list","params":{"_meta":{
        "io.modelcontextprotocol/protocolVersion":"1999-01-01","io.modelcontextprotocol/clientCapabilities":{}}}});
    let resp = Client::new()
        .post(format!("http://{}/mcp", server.addr()))
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .header("mcp-protocol-version", "1999-01-01")
        .body(bad.to_string())
        .send()
        .await
        .expect("send");
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    println!("UNSUPPORTED-VERSION over HTTP: status={status} body={text}");
    assert_eq!(
        status.as_u16(),
        400,
        "spec: HTTP 400 for an unsupported version: {text}"
    );
    let body: serde_json::Value = serde_json::from_str(&text).expect("JSON body");
    assert_eq!(body["error"]["code"], -32022);
    assert_eq!(body["error"]["data"]["requested"], "1999-01-01");
}
