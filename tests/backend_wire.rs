//! Wire test for `mcpant::backend`: a plain-function backend served by rmcp, driven by raw JSON-RPC lines.
#![cfg(feature = "backend")]

use std::sync::Arc;

use mcpant::backend::{BackendServer, ToolBackend, ToolFailure};
use rmcp::{ServiceExt, model::Tool};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, duplex};

struct Echo;

impl ToolBackend for Echo {
    fn definitions(&self) -> Vec<Tool> {
        let Value::Object(schema) =
            json!({"type":"object","properties":{"text":{"type":"string"}},"required":["text"]})
        else {
            unreachable!()
        };
        vec![Tool::new("echo", "Echo the text back", Arc::new(schema))]
    }

    async fn call(&self, name: &str, arguments: Value) -> Result<Value, ToolFailure> {
        match name {
            "echo" => arguments
                .get("text")
                .and_then(Value::as_str)
                .map(|t| json!({"text": t}))
                .ok_or_else(|| ToolFailure::bad_request("text must be a string")),
            other => Err(ToolFailure::unknown_tool(other)),
        }
    }
}

#[tokio::test]
async fn backend_lists_calls_and_reports_failures_as_error_results() {
    let (client, server) = duplex(1 << 16);
    tokio::spawn(async move {
        if let Ok(svc) = BackendServer::new(Echo)
            .with_instructions("test backend")
            .serve(server)
            .await
        {
            let _ = svc.waiting().await;
        }
    });
    let (r, w) = tokio::io::split(client);
    let mut wire = Wire {
        w,
        r: BufReader::new(r),
    };
    let init = wire
        .rpc(
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}),
        )
        .await;
    assert_eq!(init["result"]["instructions"], "test backend");
    wire.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
        .await;
    let list = wire
        .rpc(json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}))
        .await;
    assert_eq!(list["result"]["tools"][0]["name"], "echo");
    let ok = wire.rpc(json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"echo","arguments":{"text":"hi"}}})).await;
    assert_eq!(ok["result"]["isError"], false);
    assert_eq!(ok["result"]["content"][0]["text"], r#"{"text":"hi"}"#);
    let bad = wire.rpc(json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"echo","arguments":{"text":1}}})).await;
    assert_eq!(bad["result"]["isError"], true);
    assert!(
        bad["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("bad_request")
    );
    let unknown = wire.rpc(json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"nope","arguments":{}}})).await;
    assert_eq!(unknown["result"]["isError"], true);
    assert!(
        unknown["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("unknown_tool")
    );
}

struct Wire {
    w: tokio::io::WriteHalf<tokio::io::DuplexStream>,
    r: BufReader<tokio::io::ReadHalf<tokio::io::DuplexStream>>,
}

impl Wire {
    async fn send(&mut self, v: Value) {
        self.w
            .write_all(
                format!(
                    "{v}
"
                )
                .as_bytes(),
            )
            .await
            .unwrap();
    }

    async fn rpc(&mut self, v: Value) -> Value {
        self.send(v).await;
        let mut s = String::new();
        tokio::time::timeout(std::time::Duration::from_secs(3), self.r.read_line(&mut s))
            .await
            .expect("timeout")
            .unwrap();
        serde_json::from_str(&s).unwrap()
    }
}
