//! Spike for DESIGN s4b "claude/channel push" row: can an rmcp 3.11 server (a) declare the experimental
//! `claude/channel` capability and (b) push a custom-method notification (`notifications/claude/channel`) at
//! the client? Raw JSON-RPC lines, no rmcp client. rmcp is a dev-dependency only.

use rmcp::{
    Peer, RoleServer, ServerHandler, ServiceExt,
    model::{CustomNotification, ServerCapabilities, ServerConfig, ServerNotification},
    tool, tool_handler, tool_router,
};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, duplex};

#[derive(Clone)]
struct Pusher;

#[tool_router]
impl Pusher {
    #[tool(description = "Push one channel event to the client")]
    async fn push(&self, peer: Peer<RoleServer>) -> String {
        let n = CustomNotification::new(
            "notifications/claude/channel",
            Some(json!({"content": "hi (untrusted)", "meta": {"sender": "x"}})),
        );
        match peer
            .send_notification(ServerNotification::CustomNotification(n))
            .await
        {
            Ok(()) => "sent".into(),
            Err(e) => format!("error: {e}"),
        }
    }
}

#[tool_handler]
impl ServerHandler for Pusher {
    fn get_info(&self) -> ServerConfig {
        let mut caps = ServerCapabilities::builder().enable_tools().build();
        caps.experimental = Some(
            [("claude/channel".to_owned(), serde_json::Map::new())]
                .into_iter()
                .collect(),
        );
        ServerConfig::new(caps)
    }
}

async fn line(r: &mut BufReader<tokio::io::ReadHalf<tokio::io::DuplexStream>>) -> Value {
    let mut s = String::new();
    tokio::time::timeout(std::time::Duration::from_secs(3), r.read_line(&mut s))
        .await
        .expect("timeout")
        .unwrap();
    serde_json::from_str(&s).expect("json line")
}

#[tokio::test]
async fn server_declares_channel_and_pushes_custom_notification() {
    let (client, server) = duplex(1 << 16);
    tokio::spawn(async move {
        if let Ok(svc) = Pusher.serve(server).await {
            let _ = svc.waiting().await;
        }
    });
    let (r, mut w) = tokio::io::split(client);
    let mut r = BufReader::new(r);
    let mut send = async |v: Value| {
        w.write_all(format!("{v}\n").as_bytes()).await.unwrap();
    };
    send(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}))
    .await;
    let init = line(&mut r).await;
    println!("initialize result: {init}");
    assert!(init["result"]["capabilities"]["experimental"]["claude/channel"].is_object());
    send(json!({"jsonrpc":"2.0","method":"notifications/initialized"})).await;
    send(json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"push","arguments":{}}})).await;
    // The notification and the tool result may arrive in either order.
    let (a, b) = (line(&mut r).await, line(&mut r).await);
    println!("a: {a}\nb: {b}");
    let note = [&a, &b]
        .into_iter()
        .find(|m| m["method"] == "notifications/claude/channel")
        .expect("custom notification on the wire");
    assert_eq!(note["params"]["content"], "hi (untrusted)");
    assert!(note.get("id").is_none());
}
