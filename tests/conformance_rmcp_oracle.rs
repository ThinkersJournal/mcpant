//! Conformance, part 3: the differential run (DESIGN.md s4 gate). rmcp 3.11 serves a tiny calculator over an
//! in-memory pipe; this test speaks RAW JSON-RPC lines to it (never rmcp's client) and validates both what it
//! sends and what rmcp answers against each revision's vendored schema. Differences are PRINTED, not hidden:
//! run with `-- --nocapture` to read the report. rmcp is a dev-dependency oracle only; no rmcp code is copied.

use rmcp::{ServiceExt, handler::server::wrapper::Parameters, tool, tool_router};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, duplex};

#[path = "conformance_schema.rs"]
#[allow(dead_code)]
mod schema;

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

struct Wire {
    w: tokio::io::WriteHalf<tokio::io::DuplexStream>,
    r: BufReader<tokio::io::ReadHalf<tokio::io::DuplexStream>>,
}

impl Wire {
    fn start() -> Wire {
        let (client, server) = duplex(1 << 16);
        tokio::spawn(async move {
            if let Ok(svc) = Calc.serve(server).await {
                let _ = svc.waiting().await;
            }
        });
        let (r, w) = tokio::io::split(client);
        Wire {
            w,
            r: BufReader::new(r),
        }
    }
    async fn send(&mut self, v: &Value) {
        let mut line = v.to_string();
        line.push('\n');
        self.w.write_all(line.as_bytes()).await.unwrap();
    }
    /// Next line, or None on timeout (a server that stays silent is itself a finding).
    async fn recv(&mut self) -> Option<Value> {
        let mut line = String::new();
        let n = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            self.r.read_line(&mut line),
        )
        .await
        .ok()?
        .ok()?;
        if n == 0 {
            None
        } else {
            serde_json::from_str(&line).ok()
        }
    }
}

fn check(report: &mut Vec<String>, rev: &str, what: &str, def: &str, msg: &Value) {
    match schema::validate(rev, def, msg) {
        Ok(()) => report.push(format!("[{rev}] OK   {what}")),
        Err(e) => report.push(format!("[{rev}] DIFF {what}: {}", e.join(" | "))),
    }
}

/// Legacy revisions: initialize handshake, then tools/list and tools/call.
async fn run_legacy(rev: &str, report: &mut Vec<String>) {
    let mut w = Wire::start();
    let init = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "protocolVersion": rev, "capabilities": {}, "clientInfo": {"name":"mcpant-conformance","version":"0"}}});
    w.send(&init).await;
    let Some(res) = w.recv().await else {
        return report.push(format!("[{rev}] DIFF no reply to initialize"));
    };
    let result = res.get("result").cloned().unwrap_or(Value::Null);
    report.push(format!(
        "[{rev}] negotiated protocolVersion = {}",
        result["protocolVersion"]
    ));
    check(report, rev, "InitializeResult", "InitializeResult", &result);
    w.send(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
        .await;
    w.send(&json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}))
        .await;
    let list = w
        .recv()
        .await
        .and_then(|m| m.get("result").cloned())
        .unwrap_or(Value::Null);
    check(report, rev, "ListToolsResult", "ListToolsResult", &list);
    w.send(&json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"add","arguments":{"a":2,"b":3}}})).await;
    let call = w
        .recv()
        .await
        .and_then(|m| m.get("result").cloned())
        .unwrap_or(Value::Null);
    check(report, rev, "CallToolResult", "CallToolResult", &call);
    let text = call["content"][0]["text"].as_str().unwrap_or("");
    assert_eq!(text, "5", "[{rev}] add(2,3) must be 5, got {call}");
}

/// 2026-07-28: no handshake; server/discover, then per-request `_meta`.
async fn run_stateless(report: &mut Vec<String>) {
    let rev = "2026-07-28";
    let meta = json!({"io.modelcontextprotocol/protocolVersion": rev, "io.modelcontextprotocol/clientCapabilities": {}});
    let mut w = Wire::start();
    w.send(&json!({"jsonrpc":"2.0","id":1,"method":"server/discover","params":{"_meta": meta}}))
        .await;
    let Some(res) = w.recv().await else {
        return report.push(format!("[{rev}] DIFF no reply to server/discover"));
    };
    check(
        report,
        rev,
        "DiscoverResult",
        "DiscoverResult",
        &res.get("result").cloned().unwrap_or(res.clone()),
    );
    w.send(&json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{"_meta": meta}}))
        .await;
    let list = w
        .recv()
        .await
        .and_then(|m| m.get("result").cloned())
        .unwrap_or(Value::Null);
    check(report, rev, "ListToolsResult", "ListToolsResult", &list);
    w.send(&json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"_meta": meta,"name":"add","arguments":{"a":2,"b":3}}})).await;
    let call = w
        .recv()
        .await
        .and_then(|m| m.get("result").cloned())
        .unwrap_or(Value::Null);
    check(report, rev, "CallToolResult", "CallToolResult", &call);
    // An unsupported version must be answered with UnsupportedProtocolVersionError.
    let bad = json!({"io.modelcontextprotocol/protocolVersion": "1999-01-01", "io.modelcontextprotocol/clientCapabilities": {}});
    w.send(&json!({"jsonrpc":"2.0","id":4,"method":"tools/list","params":{"_meta": bad}}))
        .await;
    match w.recv().await {
        Some(m) => report.push(format!("[{rev}] unsupported-version reply: {}", m)),
        None => report.push(format!(
            "[{rev}] DIFF no reply to an unsupported protocol version"
        )),
    }
}

#[tokio::test]
async fn rmcp_311_differential_report() {
    let mut report = Vec::new();
    for rev in ["2024-11-05", "2025-03-26", "2025-06-18", "2025-11-25"] {
        run_legacy(rev, &mut report).await;
    }
    run_stateless(&mut report).await;
    println!("\n==== rmcp 3.11.0 differential report ====");
    for l in &report {
        println!("{l}");
    }
    // Negative control: the report must contain at least one OK, or the harness validated nothing.
    assert!(
        report.iter().any(|l| l.contains(" OK ")),
        "harness produced no OK line"
    );
}
