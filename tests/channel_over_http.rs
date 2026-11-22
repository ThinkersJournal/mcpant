//! The channel push over the HTTP transport on a running HTTPAnt `Server` (DESIGN s4b HTTP and channel rows): a full session
//! (initialize, initialized, standalone GET stream), then a tool call that calls `mcpant::channel::notify`. The test records WHERE the
//! event arrives (the standalone GET stream, or the POST's own response stream) because a host that pushes must know.
#![cfg(feature = "channel")]

use std::{convert::Infallible, sync::Arc, time::Duration};

use http_body_util::BodyExt;
use httpant::{Body, Client, Request, Response, Server};
use mcpant::channel;
use rmcp::{
    Peer, RoleServer, ServerHandler,
    model::{ServerCapabilities, ServerConfig},
    tool, tool_handler, tool_router,
    transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    },
};
use serde_json::json;
use tower::Service;

#[derive(Clone)]
struct Pusher;

#[tool_router]
impl Pusher {
    #[tool(description = "Push one channel event")]
    async fn push(&self, peer: Peer<RoleServer>) -> String {
        match channel::notify(&peer, "pm", "hello over http", &[]).await {
            Ok(()) => "sent".into(),
            Err(e) => format!("error: {e}"),
        }
    }
}

#[tool_handler]
impl ServerHandler for Pusher {
    fn get_info(&self) -> ServerConfig {
        let mut caps = ServerCapabilities::builder().enable_tools().build();
        caps.experimental = Some([channel::capability()].into_iter().collect());
        ServerConfig::new(caps)
    }
}

/// Read body frames until `needle` shows up or `wait` passes; returns everything read.
async fn read_until(body: &mut Body, needle: &str, wait: Duration) -> String {
    let mut got = String::new();
    let _ = tokio::time::timeout(wait, async {
        while let Some(Ok(frame)) = body.frame().await {
            if let Some(d) = frame.data_ref() {
                got.push_str(&String::from_utf8_lossy(d));
                if got.contains(needle) {
                    break;
                }
            }
        }
    })
    .await;
    got
}

#[tokio::test]
async fn a_channel_event_reaches_an_http_client_and_we_record_which_stream() {
    let svc = StreamableHttpService::new(
        || Ok(Pusher),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );
    let mount = move |req: Request| {
        let mut inner = svc.clone();
        async move {
            let r = Service::<Request>::call(&mut inner, req)
                .await
                .unwrap_or_else(|e: Infallible| match e {});
            r.map(Body::new) as Response
        }
    };
    let server = Server::new()
        .post("/mcp", mount.clone())
        .get("/mcp", mount.clone())
        .route(http::Method::DELETE, "/mcp", mount)
        .bind("127.0.0.1:0")
        .await
        .expect("bind");
    let url = format!("http://{}/mcp", server.addr());
    let c = Client::new();
    let post = |body: serde_json::Value, sid: Option<&str>| {
        let mut b = c
            .post(&url)
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .header("mcp-protocol-version", "2025-11-25");
        if let Some(s) = sid {
            b = b.header("mcp-session-id", s);
        }
        b.body(body.to_string()).send()
    };

    let init = post(
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
            "protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}),
        None,
    )
    .await
    .expect("initialize");
    let sid = init.headers()["mcp-session-id"]
        .to_str()
        .unwrap()
        .to_owned();
    let init_text = init.text().await.unwrap();
    assert!(
        init_text.contains("claude/channel"),
        "capability not advertised: {init_text}"
    );

    let ack = post(
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        Some(&sid),
    )
    .await
    .expect("initialized");
    assert_eq!(ack.status().as_u16(), 202);

    // The standalone stream: where server-initiated messages are meant to go.
    let stream = c
        .get(&url)
        .header("accept", "text/event-stream")
        .header("mcp-session-id", &sid)
        .header("mcp-protocol-version", "2025-11-25")
        .send()
        .await
        .expect("GET stream");
    assert!(stream.status().is_success(), "GET {}", stream.status());
    let mut standalone = stream.into_body();

    // Leave the standalone stream quiet first: an idle response stream must survive (httpant-server 0.7.10 sets no response-side
    // idle timeout; rmcp's own SSE keep-alive is 15 s, so nothing is sent in this window).
    tokio::time::sleep(Duration::from_secs(4)).await;

    let call = post(
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"push","arguments":{}}}),
        Some(&sid),
    )
    .await
    .expect("call");
    let mut call_body = call.into_body();
    let on_post = read_until(&mut call_body, "\"id\":2", Duration::from_secs(2)).await;
    let on_stream = read_until(
        &mut standalone,
        "notifications/claude/channel",
        Duration::from_secs(2),
    )
    .await;
    println!(
        "--- on the POST response stream:\n{on_post}\n--- on the standalone GET stream:\n{on_stream}"
    );
    let seen = on_post.contains("notifications/claude/channel")
        || on_stream.contains("notifications/claude/channel");
    assert!(seen, "the event reached neither stream");
    assert!(
        on_post.contains("\"id\":2"),
        "the tool result must come back on the POST: {on_post}"
    );

    // End the session: DELETE must be accepted, and the session id must then be refused.
    let del = c
        .delete(&url)
        .header("mcp-session-id", &sid)
        .header("mcp-protocol-version", "2025-11-25")
        .send()
        .await
        .expect("DELETE");
    println!("DELETE status={}", del.status());
    assert!(del.status().is_success(), "DELETE {}", del.status());
    let after = post(
        json!({"jsonrpc":"2.0","id":9,"method":"tools/list","params":{}}),
        Some(&sid),
    )
    .await
    .expect("after delete");
    println!(
        "after DELETE, a request with the old session id: {}",
        after.status()
    );
    assert!(
        after.status().is_client_error(),
        "a deleted session must be refused, got {}",
        after.status()
    );
}
