//! Conformance, part 3b: the differential run for resources, prompts and the `ttlMs`/`cacheScope` hints (DESIGN s4a gap list).
//! rmcp 3.11 serves one resource and one prompt over an in-memory pipe; this test speaks RAW JSON-RPC lines and validates what rmcp
//! sends against each revision's vendored schema. Differences are PRINTED (run with `-- --nocapture`), never hidden. rmcp is a
//! dev-dependency oracle only.

use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt,
    model::{
        CacheScope, ContentBlock, GetPromptRequestParams, GetPromptResponse, GetPromptResult,
        ListPromptsResult, ListResourcesResult, PaginatedRequestParams, Prompt, PromptMessage,
        ReadResourceRequestParams, ReadResourceResponse, ReadResourceResult, Resource,
        ResourceContents, Role, ServerCapabilities, ServerConfig, SubscribeRequestParams,
        SubscriptionFilter, UnsubscribeRequestParams,
    },
    service::RequestContext,
};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, duplex};

#[path = "conformance_schema.rs"]
#[allow(dead_code)]
mod schema;

#[derive(Clone)]
struct Content;

impl ServerHandler for Content {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_resources()
                .enable_resources_subscribe()
                .enable_prompts()
                .build(),
        )
    }

    async fn list_resources(
        &self,
        _r: Option<PaginatedRequestParams>,
        _c: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        Ok(ListResourcesResult::with_all_items(vec![Resource::new(
            "mem://a", "a",
        )]))
    }

    async fn read_resource(
        &self,
        _r: ReadResourceRequestParams,
        _c: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, McpError> {
        Ok(
            ReadResourceResult::new(vec![ResourceContents::text("hello", "mem://a")])
                .with_ttl_ms(60_000)
                .with_cache_scope(CacheScope::Private)
                .into(),
        )
    }

    /// rmcp leaves `subscriptions/listen` unimplemented (-32601) until the handler opts in here.
    fn accepted_subscription_filter(
        &self,
        requested: &SubscriptionFilter,
    ) -> Option<SubscriptionFilter> {
        Some(requested.clone())
    }

    async fn subscribe(
        &self,
        _r: SubscribeRequestParams,
        _c: RequestContext<RoleServer>,
    ) -> Result<(), McpError> {
        Ok(())
    }

    async fn unsubscribe(
        &self,
        _r: UnsubscribeRequestParams,
        _c: RequestContext<RoleServer>,
    ) -> Result<(), McpError> {
        Ok(())
    }

    async fn list_prompts(
        &self,
        _r: Option<PaginatedRequestParams>,
        _c: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, McpError> {
        Ok(ListPromptsResult::with_all_items(vec![Prompt::new(
            "greet",
            Some("Say hello"),
            None,
        )]))
    }

    async fn get_prompt(
        &self,
        _r: GetPromptRequestParams,
        _c: RequestContext<RoleServer>,
    ) -> Result<GetPromptResponse, McpError> {
        Ok(GetPromptResult::new(vec![PromptMessage::new(
            Role::User,
            ContentBlock::text("hello"),
        )])
        .into())
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
            if let Ok(svc) = Content.serve(server).await {
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
        self.w.write_all(format!("{v}\n").as_bytes()).await.unwrap();
    }
    async fn rpc(&mut self, v: &Value) -> Value {
        self.send(v).await;
        self.recv_value().await
    }
    async fn recv_value(&mut self) -> Value {
        let mut line = String::new();
        let n = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            self.r.read_line(&mut line),
        )
        .await
        .ok()
        .and_then(Result::ok)
        .unwrap_or(0);
        if n == 0 {
            return Value::Null;
        }
        serde_json::from_str(&line).unwrap_or(Value::Null)
    }
}

fn check(report: &mut Vec<String>, rev: &str, what: &str, def: &str, msg: &Value) {
    if msg.is_null() {
        return report.push(format!("[{rev}] DIFF {what}: no usable reply"));
    }
    let result = msg.get("result").cloned().unwrap_or(Value::Null);
    match schema::validate(rev, def, &result) {
        Ok(()) => report.push(format!("[{rev}] OK   {what}")),
        Err(e) => report.push(format!(
            "[{rev}] DIFF {what}: {} | reply: {msg}",
            e.join(" | ")
        )),
    }
}

async fn run(rev: &str, report: &mut Vec<String>) {
    let stateless = rev == "2026-07-28";
    let mut w = Wire::start();
    let meta = json!({"io.modelcontextprotocol/protocolVersion": rev, "io.modelcontextprotocol/clientCapabilities": {}});
    let params = |extra: Value| {
        let mut p = extra;
        if stateless {
            p["_meta"] = meta.clone();
        }
        p
    };
    if !stateless {
        let init = w.rpc(&json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
            "protocolVersion": rev, "capabilities": {}, "clientInfo": {"name":"mcpant-conformance","version":"0"}}})).await;
        if init.is_null() {
            return report.push(format!("[{rev}] DIFF no reply to initialize"));
        }
        w.send(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
            .await;
    }
    let cases = [
        ("resources/list", json!({}), "ListResourcesResult"),
        (
            "resources/read",
            json!({"uri":"mem://a"}),
            "ReadResourceResult",
        ),
        ("prompts/list", json!({}), "ListPromptsResult"),
        // 2026-07-28 replaced subscribe/unsubscribe with `subscriptions/listen` (checked below), so skip the old call there.
        ("prompts/get", json!({"name":"greet"}), "GetPromptResult"),
    ];
    let mut cases = Vec::from(cases);
    if !stateless {
        cases.push((
            "resources/subscribe",
            json!({"uri":"mem://a"}),
            "EmptyResult",
        ));
    }
    for (i, (method, p, def)) in cases.into_iter().enumerate() {
        let reply = w
            .rpc(&json!({"jsonrpc":"2.0","id": i + 10,"method":method,"params":params(p)}))
            .await;
        check(report, rev, method, def, &reply);
        if method == "resources/read" {
            let r = &reply["result"];
            report.push(format!(
                "[{rev}] read hints on the wire: ttlMs={} cacheScope={}",
                r.get("ttlMs").unwrap_or(&Value::Null),
                r.get("cacheScope").unwrap_or(&Value::Null)
            ));
        }
    }
    if stateless {
        listen_probe(&mut w, &meta, report).await;
    }
}

/// 2026-07-28 `subscriptions/listen`: a long-lived request. The first message back must be the acknowledged notification; there is
/// no result until the server ends the subscription. Reported as found.
async fn listen_probe(w: &mut Wire, meta: &Value, report: &mut Vec<String>) {
    let rev = "2026-07-28";
    w.send(
        &json!({"jsonrpc":"2.0","id":77,"method":"subscriptions/listen","params":{
        "_meta": meta, "notifications": {"resourceSubscriptions": ["mem://a"]}}}),
    )
    .await;
    let first = w.recv_value().await;
    report.push(format!(
        "[{rev}] subscriptions/listen first message: {first}"
    ));
    if first.is_null() {
        return report.push(format!(
            "[{rev}] DIFF subscriptions/listen: no message within 3 s"
        ));
    }
    match schema::validate(rev, "SubscriptionsAcknowledgedNotification", &first) {
        Ok(()) => report.push(format!(
            "[{rev}] OK   subscriptions/listen acknowledged notification"
        )),
        Err(e) => report.push(format!(
            "[{rev}] DIFF subscriptions/listen first message is not the acknowledgement: {}",
            e.join(" | ")
        )),
    }
}

#[tokio::test]
async fn rmcp_311_resources_prompts_and_cache_hints_report() {
    let mut report = Vec::new();
    for rev in schema::REVISIONS {
        run(rev, &mut report).await;
    }
    println!("\n==== rmcp 3.11.0 differential report: resources, prompts, cache hints ====");
    for l in &report {
        println!("{l}");
    }
    // Controls: a wrong shape must FAIL each definition, or the OK lines above assert nothing.
    for (def, bad) in [
        ("ReadResourceResult", json!({"contents": "not a list"})),
        ("ListResourcesResult", json!({"resources": 3})),
        ("ListPromptsResult", json!({"prompts": "x"})),
        ("GetPromptResult", json!({"messages": {}})),
    ] {
        assert!(
            schema::validate("2025-11-25", def, &bad).is_err(),
            "wrong shape passed {def}"
        );
    }
    assert!(
        report.iter().any(|l| l.contains(" OK ")),
        "harness produced no OK line"
    );
}
