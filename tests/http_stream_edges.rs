//! Edges of the streamable-HTTP channel push that `channel_over_http.rs` leaves open (HANDOFF Next 1): an idle standalone stream held past
//! rmcp's 15 s keep-alive, a push to a client that never opened the GET stream, and what the standalone stream does when the session ends.
//! Each test prints what it saw so DESIGN s4b can record it; assertions pin only what a host may rely on.
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
        match channel::notify(&peer, "pm", "edge", &[]).await {
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

struct Session {
    c: Client,
    url: String,
    sid: String,
    _server: httpant::RunningServer,
}

impl Session {
    async fn open() -> Self {
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
        let init = c
            .post(&url)
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .header("mcp-protocol-version", "2025-11-25")
            .body(
                json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
                    "protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"t","version":"0"}}})
                .to_string(),
            )
            .send()
            .await
            .expect("initialize");
        let sid = init.headers()["mcp-session-id"]
            .to_str()
            .unwrap()
            .to_owned();
        let _ = init.text().await;
        let s = Session {
            c,
            url,
            sid,
            _server: server,
        };
        let ack = s
            .post(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
            .await;
        assert_eq!(ack.status().as_u16(), 202);
        s
    }

    async fn post(&self, body: serde_json::Value) -> httpant::client::ClientResponse {
        self.c
            .post(&self.url)
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .header("mcp-protocol-version", "2025-11-25")
            .header("mcp-session-id", &self.sid)
            .body(body.to_string())
            .send()
            .await
            .expect("post")
    }

    async fn listen(&self) -> Body {
        let r = self
            .c
            .get(&self.url)
            .header("accept", "text/event-stream")
            .header("mcp-session-id", &self.sid)
            .header("mcp-protocol-version", "2025-11-25")
            .send()
            .await
            .expect("GET stream");
        assert!(r.status().is_success(), "GET {}", r.status());
        r.into_body()
    }

    fn call_push(&self, id: u64) -> serde_json::Value {
        json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":"push","arguments":{}}})
    }
}

/// An idle standalone stream held past rmcp's 15 s keep-alive must stay usable, and the push must still land on it afterwards.
#[tokio::test]
async fn standalone_stream_survives_past_the_keepalive_interval() {
    let s = Session::open().await;
    let mut standalone = s.listen().await;
    let quiet = read_until(
        &mut standalone,
        "notifications/claude/channel",
        Duration::from_secs(35),
    )
    .await;
    println!("35 s idle on the standalone stream saw: {quiet:?}");
    assert!(
        !quiet.contains("notifications/claude/channel"),
        "nothing was pushed yet"
    );

    let call = s.post(s.call_push(2)).await;
    let mut call_body = call.into_body();
    let on_post = read_until(&mut call_body, "\"id\":2", Duration::from_secs(2)).await;
    let on_stream = read_until(
        &mut standalone,
        "notifications/claude/channel",
        Duration::from_secs(3),
    )
    .await;
    println!("after 35 s idle: POST saw {on_post:?}; standalone saw {on_stream:?}");
    assert!(
        on_post.contains("\"id\":2"),
        "tool result must return on the POST: {on_post}"
    );
    assert!(
        on_stream.contains("notifications/claude/channel")
            || on_post.contains("notifications/claude/channel"),
        "the event reached neither stream after a long idle"
    );
}

/// A client that never opens the GET stream: record what `notify` returns and where (if anywhere) the event goes.
#[tokio::test]
async fn push_to_a_client_that_never_opened_the_get_stream() {
    let s = Session::open().await;
    let call = s.post(s.call_push(2)).await;
    let mut call_body = call.into_body();
    let on_post = read_until(&mut call_body, "\"id\":2", Duration::from_secs(3)).await;
    println!("no GET stream: POST response saw {on_post:?}");
    assert!(
        on_post.contains("\"id\":2"),
        "the tool call itself must still answer: {on_post}"
    );
    // Whether `sent` or `error:` comes back, and whether the event rode the POST, is the recorded finding, not an assertion.
}

/// What the standalone stream does when the session is deleted: it must end (not hang), so a client can notice.
#[tokio::test]
async fn standalone_stream_ends_when_the_session_is_deleted() {
    let s = Session::open().await;
    let mut standalone = s.listen().await;
    let del =
        s.c.delete(&s.url)
            .header("mcp-session-id", &s.sid)
            .header("mcp-protocol-version", "2025-11-25")
            .send()
            .await
            .expect("DELETE");
    assert!(del.status().is_success(), "DELETE {}", del.status());
    let ended = tokio::time::timeout(Duration::from_secs(5), async {
        let mut seen = String::new();
        while let Some(f) = standalone.frame().await {
            match f {
                Ok(fr) => {
                    if let Some(d) = fr.data_ref() {
                        seen.push_str(&String::from_utf8_lossy(d));
                    }
                }
                Err(e) => return format!("error after {seen:?}: {e}"),
            }
        }
        format!("clean end after {seen:?}")
    })
    .await;
    println!("standalone stream after DELETE: {ended:?}");
    assert!(
        ended.is_ok(),
        "the standalone stream hung after the session was deleted"
    );
}
