# MCPAnt

A host layer for [Model Context Protocol](https://modelcontextprotocol.io) servers in Rust, over a pinned
[`rmcp`](https://crates.io/crates/rmcp) (3.12 at this writing; MSRV **1.88**, rmcp's own floor). Each layer is an
opt-in cargo feature, none on by default. MCPAnt does not reimplement the protocol: rmcp is a correct implementation
of all five spec revisions on the tools path (our conformance suite, `tests/conformance_rmcp_oracle*.rs`, validates
it against each revision's schema), so MCPAnt adds what rmcp lacks and our servers keep rewriting.

## Hello, world

A stdio MCP server with one tool (this is `examples/hello.rs`, built by CI):

```sh
cargo new greeter && cd greeter
cargo add mcpant --features backend
cargo add tokio --features macros,rt-multi-thread
cargo add serde_json
```

```rust
use mcpant::backend::{BackendServer, ToolBackend, ToolDefinition, ToolFailure, tool};
use serde_json::{Value, json};

struct Greeter;

impl ToolBackend for Greeter {
    fn definitions(&self) -> Vec<ToolDefinition> {
        vec![tool(
            "greet",
            "Say hello to someone",
            json!({
                "type": "object",
                "properties": {"name": {"type": "string", "description": "Who to greet"}},
                "required": ["name"]
            }),
        )]
    }

    async fn call(&self, name: &str, args: Value) -> Result<Value, ToolFailure> {
        match name {
            "greet" => {
                let who = args["name"]
                    .as_str()
                    .ok_or_else(|| ToolFailure::bad_request("name must be a string"))?;
                Ok(json!(format!("Hello, {who}!")))
            }
            other => Err(ToolFailure::unknown_tool(other)),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    BackendServer::new(Greeter)
        .with_server_info("greeter", env!("CARGO_PKG_VERSION"))
        .serve_stdio()
        .await
}
```

`cargo run`, then point an MCP client at the binary. Tool failures come back as `isError` results with a stable
`code`; stdout carries only the protocol, so log to stderr. Measured 2026-11-26 from `cargo new` in a clean project
(registry warm, path dependency until the first publish): 5 commands plus one paste, 49 s to a running server.

## Features

| Feature | What |
|---|---|
| `backend` | Write tools as plain functions (`ToolBackend`), serve them over rmcp (stdio in one call, or any rmcp transport). |
| `truncate` | Budgeted output with an honest `truncated` flag (lifted from sageant, credited). |
| `channel` | Declare `claude/channel` and push escaped, untrusted-labelled events to Claude Code. |
| `limits` | Per-call timeout that cancels the call, and a concurrency cap (implies `backend`). |
| `namespace` | `<server>__<tool>` names for hosts that aggregate servers. |

HTTP: rmcp's `StreamableHttpService` mounts on HTTPAnt (`tests/spike_httpant_server.rs`, `tests/channel_over_http.rs`);
a keyed rate limit is shown in `tests/http_rate_limit.rs`. These are tests, not a library surface yet.

## Not built (deferred)

- **An own protocol core** (per-spec-version modules). CireSnave ruled for the host layer over rmcp on 2026-11-20
  (board 473, option B); the core waits for a consumer that needs something rmcp cannot do (`docs/DESIGN.md` 4b).
- **A client half**, and a one-call HTTP server (`serve_http`): not started.
- **MRTR `inputRequests`, a real Claude Code client on the channel, minutes-long SSE streams, httpant's request-side
  timeouts**: unexercised (`docs/DESIGN.md` 4c).

## Publishing status

Only the 0.0.0 placeholder is on crates.io; the PM publishes the first real 0.x. Until then depend on a checkout.
`cargo deny` carries one standing ignore, `RUSTSEC-2026-0543` (chrono unmaintained, pulled by rmcp and schemars;
re-checked at rmcp 3.12.0: still pulled), expiring 2026-12-17.

Dual-licensed MIT OR Apache-2.0.
