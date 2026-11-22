//! Per-client rate limiting belongs to the HTTP mount, not the backend (`limits.rs` module note, DESIGN s4b "Limits"). This test is the
//! worked example: a wrapper around the rmcp mount that keys a fixed-window counter on a client identity and answers `429` with
//! `Retry-After` once the budget is spent. `tower::limit::RateLimit` cannot do this by itself: it is one global bucket for the whole
//! service and has no notion of a client, so a keyed limiter has to sit in front of it.
//!
//! The identity here is the `x-client-id` request header; a real host takes it from its authenticated principal (never from a header
//! the client chooses, unless an upstream proxy sets it).

use std::{
    collections::HashMap,
    convert::Infallible,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use httpant::{Body, Client, Request, Response, Server};
use rmcp::{
    ServerHandler,
    model::{ServerCapabilities, ServerConfig},
    transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    },
};
use serde_json::json;
use tower::Service;

#[derive(Clone)]
struct Empty;

impl ServerHandler for Empty {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
    }
}

/// Fixed-window counter per client key.
#[derive(Clone)]
struct PerClient {
    max: u32,
    window: Duration,
    seen: Arc<Mutex<HashMap<String, (Instant, u32)>>>,
}

impl PerClient {
    fn new(max: u32, window: Duration) -> Self {
        Self { max, window, seen: Arc::default() }
    }

    /// `Ok` to proceed, `Err(retry_after_secs)` when the client's budget for this window is spent.
    fn admit(&self, key: &str) -> Result<(), u64> {
        let now = Instant::now();
        let mut m = self.seen.lock().unwrap();
        let e = m.entry(key.to_owned()).or_insert((now, 0));
        if now.duration_since(e.0) >= self.window {
            *e = (now, 0);
        }
        if e.1 >= self.max {
            return Err((self.window - now.duration_since(e.0)).as_secs().max(1));
        }
        e.1 += 1;
        Ok(())
    }
}

#[tokio::test]
async fn a_client_over_budget_gets_429_and_another_client_is_unaffected() {
    let svc = StreamableHttpService::new(
        || Ok(Empty),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );
    let limiter = PerClient::new(2, Duration::from_secs(60));
    let mount = move |req: Request| {
        let mut inner = svc.clone();
        let limiter = limiter.clone();
        async move {
            let key = req
                .headers()
                .get("x-client-id")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("anonymous")
                .to_owned();
            if let Err(secs) = limiter.admit(&key) {
                return http::Response::builder()
                    .status(429)
                    .header("retry-after", secs.to_string())
                    .body(Body::from_bytes("rate limit exceeded"))
                    .unwrap() as Response;
            }
            let r = Service::<Request>::call(&mut inner, req)
                .await
                .unwrap_or_else(|e: Infallible| match e {});
            r.map(Body::new) as Response
        }
    };
    let server = Server::new().post("/mcp", mount).bind("127.0.0.1:0").await.expect("bind");
    let url = format!("http://{}/mcp", server.addr());
    let c = Client::new();
    let init = |who: &'static str| {
        c.post(&url)
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .header("mcp-protocol-version", "2025-11-25")
            .header("x-client-id", who)
            .body(
                json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
                    "protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"t","version":"0"}}})
                .to_string(),
            )
            .send()
    };

    let mut a = Vec::new();
    for _ in 0..3 {
        a.push(init("alice").await.expect("request").status().as_u16());
    }
    println!("alice, budget 2 per window: {a:?}");
    assert_eq!(&a[..2], &[200, 200], "the first two are within budget");
    assert_eq!(a[2], 429, "the third is over budget");

    let over = init("alice").await.expect("request");
    assert_eq!(over.status().as_u16(), 429);
    assert!(over.headers().contains_key("retry-after"));

    let b = init("bob").await.expect("request").status().as_u16();
    println!("bob, same window: {b}");
    assert_eq!(b, 200, "another client has its own budget");
}
