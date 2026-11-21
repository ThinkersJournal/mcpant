//! A transport-free tool backend, served over rmcp.
//!
//! Implement [`ToolBackend`] as plain functions over your own state, `(name, arguments) -> Result<Value, ToolFailure>`, and
//! test it without a transport; [`BackendServer`] is the thin rmcp wiring (list the definitions, run the call, report a
//! failure as an MCP error result with a stable `code`). DESIGN.md section 4b, "Tool registry + dispatch": rmcp's own
//! `ToolRouter` stays the choice when tools are plain methods; this is for a backend that already exists as a library.
//!
//! The shape is lifted from the `sageant-mcp` crate (sageant, MIT OR Apache-2.0, credit the sageant project; DESIGN.md
//! section 7 has the per-file provenance trace). The trait, [`ToolFailure`] and the async signature are new here.

use std::future::Future;
use std::sync::Arc;

use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, ListToolsResult,
    PaginatedRequestParams, ServerCapabilities, ServerConfig, Tool,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};
use serde_json::{Value, json};

/// Why a tool call failed: a short stable `code` for the client and a human `message`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolFailure {
    /// Stable, machine-readable (`unknown_tool`, `bad_request`, ...).
    pub code: String,
    /// For a person; never put untrusted stored text here.
    pub message: String,
}

impl ToolFailure {
    /// A failure with this `code` and `message`.
    #[must_use]
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }

    /// The tool is not one the backend offers.
    #[must_use]
    pub fn unknown_tool(name: &str) -> Self {
        Self::new("unknown_tool", format!("no tool named {name}"))
    }

    /// A missing, mistyped or malformed argument.
    #[must_use]
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new("bad_request", message)
    }

    /// The JSON the client gets: `{"error": {"code", "message"}}`.
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({"error": {"code": self.code, "message": self.message}})
    }
}

/// Tools as plain functions over your own state.
pub trait ToolBackend: Send + Sync + 'static {
    /// The tools offered, with their JSON Schemas.
    fn definitions(&self) -> Vec<Tool>;

    /// Run one tool. An `Err` becomes an MCP error result (`isError: true`), not a protocol error.
    fn call(
        &self,
        name: &str,
        arguments: Value,
    ) -> impl Future<Output = Result<Value, ToolFailure>> + Send;
}

/// Serves a [`ToolBackend`] as an rmcp server.
pub struct BackendServer<B> {
    backend: Arc<B>,
    instructions: Option<String>,
}

impl<B> Clone for BackendServer<B> {
    fn clone(&self) -> Self {
        Self {
            backend: Arc::clone(&self.backend),
            instructions: self.instructions.clone(),
        }
    }
}

impl<B: ToolBackend> BackendServer<B> {
    /// Serve `backend`.
    #[must_use]
    pub fn new(backend: B) -> Self {
        Self {
            backend: Arc::new(backend),
            instructions: None,
        }
    }

    /// The `instructions` text shown to the client at initialisation.
    #[must_use]
    pub fn with_instructions(mut self, text: impl Into<String>) -> Self {
        self.instructions = Some(text.into());
        self
    }
}

impl<B: ToolBackend> ServerHandler for BackendServer<B> {
    fn get_info(&self) -> ServerConfig {
        let info = ServerConfig::new(ServerCapabilities::builder().enable_tools().build());
        match &self.instructions {
            Some(text) => info.with_instructions(text.clone()),
            None => info,
        }
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult::with_all_items(self.backend.definitions()))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        let arguments = Value::Object(request.arguments.unwrap_or_default());
        let result = match self.backend.call(&request.name, arguments).await {
            Ok(v) => CallToolResult::success(vec![ContentBlock::text(v.to_string())]),
            Err(e) => CallToolResult::error(vec![ContentBlock::text(e.to_json().to_string())]),
        };
        Ok(result.into())
    }
}
