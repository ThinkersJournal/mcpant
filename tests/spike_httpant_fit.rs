//! Spike (DESIGN s4b, HTTP row): does rmcp 3.11's `StreamableHttpService` mount behind HTTPAnt's
//! `Request`/`Response`/`Body` types? Answered by compiling the adapter and sending one real request through it.
//! HTTPAnt is a crates.io dev-dependency (httpant-core), per the portfolio's no-path-deps rule.

use httpant_core::{Body, Request, Response};
use rmcp::{
    handler::server::wrapper::Parameters,
    tool, tool_router,
    transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    },
};
use std::{convert::Infallible, sync::Arc};
use tower::{Service, ServiceExt};

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
async fn rmcp_streamable_http_mounts_on_httpant_types() {
    let rmcp_svc = StreamableHttpService::new(
        || Ok(Calc),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );
    // The adapter under test: HTTPAnt Request in, HTTPAnt Response out.
    let mut svc = tower::service_fn(move |req: Request| {
        let mut inner = rmcp_svc.clone();
        async move {
            let resp = Service::<Request>::call(&mut inner, req).await.unwrap();
            Ok::<Response, Infallible>(resp.map(Body::new))
        }
    });
    let body = serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"spike","version":"0"}}});
    let req: Request = http::Request::builder()
        .method("POST")
        .uri("http://localhost/mcp")
        .header("host", "localhost")
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .body(Body::new(http_body_util::Full::new(
            httpant_core::Bytes::from(body.to_string()),
        )))
        .unwrap();
    let resp = svc.ready().await.unwrap().call(req).await.unwrap();
    let status = resp.status();
    let ctype = resp
        .headers()
        .get("content-type")
        .map(|v| v.to_str().unwrap().to_owned());
    let bytes = resp
        .into_body()
        .bytes(1 << 20)
        .await
        .map(|b| String::from_utf8_lossy(&b).into_owned());
    println!("status={status} content-type={ctype:?}\nbody={bytes:?}");
    assert!(
        status.is_success(),
        "initialize through the adapter failed: {status}"
    );
    assert!(
        bytes.unwrap().contains("protocolVersion"),
        "no InitializeResult in the body"
    );
}
