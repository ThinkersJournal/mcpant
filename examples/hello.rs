//! The README's hello-world: a stdio MCP server with one tool. Run: `cargo run --example hello --features backend`.
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
