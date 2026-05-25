//! MCP-backed [`dasclaw_runtime::ToolExecutor`] bridge.
//!
//! ADR-153 step 7. Headless callers (e.g. `dasclaw-cli`) hold one or more
//! [`McpClient`] instances and want to advertise the union of their tools
//! to the LLM as [`ToolDefinition`]s and dispatch each tool call back to
//! the right server. This module is that bridge.
//!
//! # Design
//!
//! - Each tool from each client is recorded as a single [`McpRoute`].
//! - Tool names are **qualified** as `<server_name>_<tool_name>` to
//!   avoid collisions between two servers exposing a tool with the same
//!   name. This mirrors ironclaw's `tools::mcp::client_tool::create_tools_for`.
//! - [`Self::execute`] always returns `Ok(ToolResult)`. Unknown tools and
//!   transport errors are surfaced as `ToolResult { is_error: true }` so
//!   the model can recover; nothing in this layer bubbles up as
//!   [`HostError`].
//!
//! # Testing seam
//!
//! [`McpToolExecutor::from_routes`] accepts a pre-discovered routing
//! table so unit tests can wire a `McpClient` backed by a fake
//! `McpTransport` without performing a real `tools/list` round-trip.
//! [`McpToolExecutor::from_clients`] is the production constructor and
//! calls `list_tools()` on every client.

use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use dasclaw_core::messages::{ToolCall, ToolDefinition, ToolResult};
use dasclaw_core::traits::HostError;
use dasclaw_runtime::ToolExecutor;
use dasclaw_tool::ToolError;

use crate::client::McpClient;
use crate::protocol::{CallToolResult, ContentBlock, McpTool};

/// A single (client, tool) pair tracked by [`McpToolExecutor`].
#[derive(Clone)]
struct McpRoute {
    client: Arc<McpClient>,
    /// The tool name as advertised by the MCP server, *without* the
    /// `<server_name>_` prefix. This is what `McpClient::call_tool`
    /// expects.
    tool_name: String,
    /// The fully-qualified tool definition advertised to the LLM.
    definition: ToolDefinition,
}

/// [`ToolExecutor`] that dispatches tool calls to one or more MCP
/// servers via [`McpClient`].
#[derive(Clone, Default)]
pub struct McpToolExecutor {
    /// Qualified tool name -> route. `BTreeMap` gives a stable ordering
    /// for [`Self::definitions`] which keeps test assertions readable.
    routes: BTreeMap<String, McpRoute>,
}

impl McpToolExecutor {
    /// Build an empty executor. Mostly useful for tests; production
    /// callers should prefer [`Self::from_clients`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Discover tools from each client (via `list_tools`) and build a
    /// routing table.
    ///
    /// If two servers happen to expose the same qualified name (i.e. the
    /// same `server_name` *and* `tool_name`), the **last** client wins.
    /// Collisions across different `server_name`s are impossible by
    /// construction because the qualified name embeds the server.
    pub async fn from_clients(clients: Vec<Arc<McpClient>>) -> Result<Self, ToolError> {
        let mut exec = Self::new();
        for client in clients {
            exec.add_client(client).await?;
        }
        Ok(exec)
    }

    /// Discover tools from a single client and merge them into the
    /// routing table.
    pub async fn add_client(&mut self, client: Arc<McpClient>) -> Result<(), ToolError> {
        let tools = client.list_tools().await?;
        let server_name = client.server_name().to_string();
        for tool in tools {
            let qualified = qualify_tool_name(&server_name, &tool.name);
            let definition = build_definition(&qualified, &tool);
            self.routes.insert(
                qualified,
                McpRoute {
                    client: client.clone(),
                    tool_name: tool.name,
                    definition,
                },
            );
        }
        Ok(())
    }

    /// Build a routing table directly from `(client, tool)` pairs.
    ///
    /// Test seam: lets unit tests skip the `list_tools` round-trip. The
    /// `server_name` used to qualify each tool comes from
    /// [`McpClient::server_name`].
    pub fn from_routes(routes: impl IntoIterator<Item = (Arc<McpClient>, McpTool)>) -> Self {
        let mut exec = Self::new();
        for (client, tool) in routes {
            let server_name = client.server_name().to_string();
            let qualified = qualify_tool_name(&server_name, &tool.name);
            let definition = build_definition(&qualified, &tool);
            exec.routes.insert(
                qualified,
                McpRoute {
                    client,
                    tool_name: tool.name,
                    definition,
                },
            );
        }
        exec
    }

    /// All advertised tool definitions, sorted by qualified name.
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.routes
            .values()
            .map(|route| route.definition.clone())
            .collect()
    }

    /// Number of registered tools across all clients.
    pub fn len(&self) -> usize {
        self.routes.len()
    }

    /// Whether any tool is registered.
    pub fn is_empty(&self) -> bool {
        self.routes.is_empty()
    }
}

#[async_trait]
impl ToolExecutor for McpToolExecutor {
    async fn execute(&self, call: &ToolCall) -> Result<ToolResult, HostError> {
        let Some(route) = self.routes.get(&call.name) else {
            return Ok(ToolResult {
                tool_call_id: call.id.clone(),
                name: call.name.clone(),
                content: format!("unknown MCP tool: {}", call.name),
                is_error: true,
            });
        };

        match route
            .client
            .call_tool(&route.tool_name, call.arguments.clone())
            .await
        {
            Ok(result) => Ok(map_call_result(call, result)),
            Err(err) => Ok(ToolResult {
                tool_call_id: call.id.clone(),
                name: call.name.clone(),
                content: format!("MCP transport error: {err}"),
                is_error: true,
            }),
        }
    }
}

fn qualify_tool_name(server_name: &str, tool_name: &str) -> String {
    format!("{server_name}_{tool_name}")
}

fn build_definition(qualified_name: &str, tool: &McpTool) -> ToolDefinition {
    ToolDefinition {
        name: qualified_name.to_string(),
        description: tool.description.clone(),
        parameters: tool.input_schema.clone(),
    }
}

fn map_call_result(call: &ToolCall, result: CallToolResult) -> ToolResult {
    let content = result
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            ContentBlock::Resource {
                text: Some(text), ..
            } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    ToolResult {
        tool_call_id: call.id.clone(),
        name: call.name.clone(),
        content,
        is_error: result.is_error,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex as StdMutex;

    use serde_json::json;

    use super::*;
    use crate::protocol::{McpRequest, McpResponse};
    use crate::transport::McpTransport;

    /// Minimal MCP transport stub. Returns canned responses in FIFO order
    /// and records nothing about the request beyond the count consumed.
    /// Mirrors the pattern used by `client::tests::MockTransport` but is
    /// scoped to this module so unit tests stay self-contained.
    struct StubTransport {
        responses: StdMutex<Vec<McpResponse>>,
    }

    impl StubTransport {
        fn new(responses: Vec<McpResponse>) -> Self {
            Self {
                responses: StdMutex::new(responses),
            }
        }
    }

    #[async_trait]
    impl McpTransport for StubTransport {
        async fn send(
            &self,
            _request: &McpRequest,
            _headers: &HashMap<String, String>,
        ) -> Result<McpResponse, ToolError> {
            let mut q = self
                .responses
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            if q.is_empty() {
                return Err(ToolError::ExternalService(
                    "no more stub responses".to_string(),
                ));
            }
            Ok(q.remove(0))
        }

        async fn shutdown(&self) -> Result<(), ToolError> {
            Ok(())
        }
    }

    fn ok_response(id: u64, result: serde_json::Value) -> McpResponse {
        McpResponse {
            jsonrpc: "2.0".to_string(),
            id: Some(id),
            result: Some(result),
            error: None,
        }
    }

    fn build_client(server_name: &str, transport: Arc<dyn McpTransport>) -> Arc<McpClient> {
        Arc::new(McpClient::new_with_transport(
            server_name,
            transport,
            None,
            None,
            "test-user",
            None,
        ))
    }

    fn sample_tool(name: &str) -> McpTool {
        McpTool {
            name: name.to_string(),
            description: format!("demo tool {name}"),
            input_schema: json!({
                "type": "object",
                "properties": { "value": { "type": "string" } },
                "required": ["value"],
            }),
            annotations: None,
        }
    }

    fn tool_call(name: &str, args: serde_json::Value) -> ToolCall {
        ToolCall {
            id: format!("call_{name}"),
            name: name.to_string(),
            arguments: args,
            reasoning: None,
        }
    }

    #[tokio::test]
    async fn req_dasclaw_mcp_executor_b1_empty_executor_reports_unknown_tool() {
        let exec = McpToolExecutor::new();
        let result = exec
            .execute(&tool_call("anything", json!({})))
            .await
            .expect("execute should not fail at the host layer");
        assert!(result.is_error);
        assert!(result.content.contains("unknown MCP tool"));
        assert_eq!(result.name, "anything");
        assert!(exec.definitions().is_empty());
        assert!(exec.is_empty());
    }

    #[tokio::test]
    async fn req_dasclaw_mcp_executor_b2_definitions_are_qualified_and_sorted() {
        // Two clients (alpha, beta) each exposing a `ping` tool. The
        // qualified names should be `alpha_ping` < `beta_ping`.
        let transport_a: Arc<dyn McpTransport> =
            Arc::new(StubTransport::new(vec![]));
        let transport_b: Arc<dyn McpTransport> =
            Arc::new(StubTransport::new(vec![]));
        let client_a = build_client("alpha", transport_a);
        let client_b = build_client("beta", transport_b);

        let exec = McpToolExecutor::from_routes(vec![
            (client_b.clone(), sample_tool("ping")),
            (client_a.clone(), sample_tool("ping")),
        ]);

        let defs = exec.definitions();
        assert_eq!(exec.len(), 2);
        assert_eq!(defs[0].name, "alpha_ping");
        assert_eq!(defs[1].name, "beta_ping");
        // Schema is forwarded verbatim from the MCP tool descriptor.
        assert_eq!(
            defs[0].parameters,
            json!({
                "type": "object",
                "properties": { "value": { "type": "string" } },
                "required": ["value"],
            })
        );
    }

    #[tokio::test]
    async fn req_dasclaw_mcp_executor_b3_dispatches_call_and_maps_text_content() {
        // Wire order observed inside `McpClient::initialize` +
        // `call_tool`: (1) initialize request, (2) `notifications/initialized`
        // notification (response body is ignored but the stub still has to
        // hand one back), (3) the actual `tools/call`.
        let transport: Arc<dyn McpTransport> = Arc::new(StubTransport::new(vec![
            ok_response(1, json!({ "protocolVersion": "2024-11-05" })),
            ok_response(2, json!({})),
            ok_response(
                3,
                json!({
                    "content": [
                        { "type": "text", "text": "first" },
                        { "type": "text", "text": "second" }
                    ],
                    "is_error": false
                }),
            ),
        ]));
        let client = build_client("svc", transport);
        let exec = McpToolExecutor::from_routes(vec![(client, sample_tool("ping"))]);

        let result = exec
            .execute(&tool_call("svc_ping", json!({ "value": "hi" })))
            .await
            .expect("execute should not error at host layer");

        assert!(!result.is_error, "expected success, got {result:?}");
        assert_eq!(result.content, "first\nsecond");
        assert_eq!(result.tool_call_id, "call_svc_ping");
        assert_eq!(result.name, "svc_ping");
    }

    #[tokio::test]
    async fn req_dasclaw_mcp_executor_b4_server_error_becomes_tool_error_result() {
        // initialize ack, notification ack, then a tools/call that returns
        // is_error=true.
        let transport: Arc<dyn McpTransport> = Arc::new(StubTransport::new(vec![
            ok_response(1, json!({ "protocolVersion": "2024-11-05" })),
            ok_response(2, json!({})),
            ok_response(
                3,
                json!({
                    "content": [{ "type": "text", "text": "boom" }],
                    "is_error": true
                }),
            ),
        ]));
        let client = build_client("svc", transport);
        let exec = McpToolExecutor::from_routes(vec![(client, sample_tool("ping"))]);

        let result = exec
            .execute(&tool_call("svc_ping", json!({ "value": "x" })))
            .await
            .expect("execute should not error at host layer");

        assert!(result.is_error);
        assert_eq!(result.content, "boom");
    }

    #[tokio::test]
    async fn req_dasclaw_mcp_executor_b5_transport_failure_becomes_tool_error_result() {
        // initialize ack + notification ack are provided so that
        // `McpClient::initialize` succeeds; the subsequent `tools/call`
        // exhausts the stub and surfaces an ExternalService error which
        // the executor must translate into a tool-level failure rather
        // than propagating as HostError.
        let transport: Arc<dyn McpTransport> = Arc::new(StubTransport::new(vec![
            ok_response(1, json!({ "protocolVersion": "2024-11-05" })),
            ok_response(2, json!({})),
        ]));
        let client = build_client("svc", transport);
        let exec = McpToolExecutor::from_routes(vec![(client, sample_tool("ping"))]);

        let result = exec
            .execute(&tool_call("svc_ping", json!({ "value": "x" })))
            .await
            .expect("execute must not surface HostError for transport failures");

        assert!(result.is_error);
        assert!(
            result.content.contains("MCP")
                || result.content.contains("transport")
                || result.content.contains("stub"),
            "unexpected error content: {}",
            result.content
        );
    }
}
