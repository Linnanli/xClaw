//! Ironclaw `Tool` trait adapter for MCP servers.
//!
//! This module hosts the GUI-/runtime-side glue between the host-agnostic
//! [`dasclaw_mcp::client::McpClient`] (PR 6-γ, #666) and ironclaw's local
//! [`crate::tools::tool::Tool`] trait. It owns:
//!
//! - [`McpToolWrapper`]: implements [`Tool`] for a single MCP tool by
//!   delegating `execute` to `McpClient::call_tool`.
//! - [`create_tools_for`]: enumerate an MCP server's tools and wrap each
//!   in a `Tool` impl ready for the host tool registry.
//! - [`ironclaw_refresh_fn`]: factory for the host-side
//!   [`RefreshAccessTokenFn`] hook, which calls into
//!   [`crate::tools::mcp::auth::refresh_access_token`] (the orchestration
//!   wrapper around the optional OAuth proxy and the direct RFC 6749
//!   refresh path).
//! - [`strip_top_level_nulls`]: defensive sanitization of LLM-emitted
//!   parameters (private to the wrapper but exported via tests).
//!
//! Migrated from `desktop-client/ironclaw/src/tools/mcp/client.rs` in
//! F3.2 phase 3 PR 6-γ (#661, #666). The protocol/state-machine parts
//! moved to `dasclaw_mcp::client`; this file keeps the host-specific
//! adapter so the `Tool` trait and `JobContext` types stay in ironclaw.

use std::sync::Arc;

use async_trait::async_trait;

use crate::context::JobContext;
use crate::tools::mcp::protocol::McpTool;
use crate::tools::tool::{ApprovalRequirement, Tool, ToolError, ToolOutput};
use dasclaw_mcp::client::{McpClient, RefreshAccessTokenFn};

/// Wrapper that implements Tool for an MCP tool.
struct McpToolWrapper {
    tool: McpTool,
    prefixed_name: String,
    client: Arc<McpClient>,
}

/// Enumerate the MCP server's tools and wrap each as a [`Tool`] impl.
///
/// Replaces the former `McpClient::create_tools` method (PR 6-γ, #666):
/// the wrapper now lives in the ironclaw host crate alongside the [`Tool`]
/// trait, so the dasclaw_mcp client no longer needs to know about it.
pub async fn create_tools_for(client: Arc<McpClient>) -> Result<Vec<Arc<dyn Tool>>, ToolError> {
    let mcp_tools = client.list_tools().await?;
    let server_name = client.server_name().to_string();
    Ok(mcp_tools
        .into_iter()
        .map(|t| {
            let prefixed_name = format!("{}_{}", server_name, t.name);
            Arc::new(McpToolWrapper {
                tool: t,
                prefixed_name,
                client: client.clone(),
            }) as Arc<dyn Tool>
        })
        .collect())
}

/// Build the ironclaw OAuth-proxy-aware refresh hook installed by
/// [`crate::tools::mcp::factory::create_client_from_config`].
///
/// The closure delegates to [`crate::tools::mcp::auth::refresh_access_token`],
/// which transparently routes through the optional OAuth proxy when one is
/// configured (see `cli::oauth_defaults`) and falls back to the direct
/// RFC 6749 refresh otherwise.
pub fn ironclaw_refresh_fn() -> RefreshAccessTokenFn {
    Arc::new(|config, secrets, user_id| {
        Box::pin(async move {
            crate::tools::mcp::auth::refresh_access_token(&config, &secrets, &user_id).await
        })
    })
}

#[async_trait]
impl Tool for McpToolWrapper {
    fn name(&self) -> &str {
        &self.prefixed_name
    }
    fn description(&self) -> &str {
        &self.tool.description
    }
    fn parameters_schema(&self) -> serde_json::Value {
        self.tool.input_schema.clone()
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        _ctx: &JobContext,
    ) -> Result<ToolOutput, ToolError> {
        let start = std::time::Instant::now();

        // Strip top-level null values before forwarding — LLMs often emit
        // `"field": null` for optional params, but many MCP servers reject
        // explicit nulls for fields that should simply be absent.
        let params = strip_top_level_nulls(params);

        let result = self.client.call_tool(&self.tool.name, params).await?;
        let content: String = result
            .content
            .iter()
            .filter_map(|b| b.as_text())
            .collect::<Vec<_>>()
            .join("\n");
        if result.is_error {
            return Err(ToolError::ExecutionFailed(content));
        }
        Ok(ToolOutput::text(content, start.elapsed()))
    }

    fn requires_sanitization(&self) -> bool {
        true
    }

    fn requires_approval(&self, _params: &serde_json::Value) -> ApprovalRequirement {
        if self.tool.requires_approval() {
            ApprovalRequirement::UnlessAutoApproved
        } else {
            ApprovalRequirement::Never
        }
    }
}

/// Remove top-level keys whose value is JSON null from an object.
///
/// LLMs frequently emit `"field": null` for optional parameters.  Many MCP
/// servers (e.g. Notion) treat an explicit `null` as an invalid value for
/// optional fields that should simply be absent.  Stripping these before
/// forwarding avoids 400-class rejections from strict servers.
fn strip_top_level_nulls(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let filtered = map.into_iter().filter(|(_, v)| !v.is_null()).collect();
            serde_json::Value::Object(filtered)
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_mcp_tool(destructive: bool) -> McpTool {
        use crate::tools::mcp::protocol::McpToolAnnotations;
        McpTool {
            name: "do_thing".to_string(),
            description: "Does a thing".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "input": {"type": "string"}
                }
            }),
            annotations: if destructive {
                Some(McpToolAnnotations {
                    destructive_hint: true,
                    side_effects_hint: false,
                    read_only_hint: false,
                    execution_time_hint: None,
                })
            } else {
                None
            },
        }
    }

    #[test]
    fn test_strip_top_level_nulls_removes_null_fields() {
        let input = serde_json::json!({
            "query": "search term",
            "sort": null,
            "filter": null,
            "page_size": 10
        });
        let result = strip_top_level_nulls(input);
        let obj = result.as_object().unwrap();
        assert_eq!(obj.len(), 2);
        assert_eq!(obj["query"], "search term");
        assert_eq!(obj["page_size"], 10);
        assert!(!obj.contains_key("sort"));
        assert!(!obj.contains_key("filter"));
    }

    #[test]
    fn test_strip_top_level_nulls_preserves_non_objects() {
        let input = serde_json::json!("just a string");
        let result = strip_top_level_nulls(input.clone());
        assert_eq!(result, input);
    }

    #[test]
    fn test_strip_top_level_nulls_preserves_nested_nulls() {
        let input = serde_json::json!({
            "outer": { "inner": null },
            "top_null": null
        });
        let result = strip_top_level_nulls(input);
        let obj = result.as_object().unwrap();
        assert_eq!(obj.len(), 1);
        assert!(obj["outer"]["inner"].is_null());
    }

    #[test]
    fn test_mcp_tool_wrapper_name_is_prefixed() {
        let client = Arc::new(McpClient::new("http://localhost:8080"));
        let wrapper = McpToolWrapper {
            tool: make_test_mcp_tool(false),
            prefixed_name: "mcp__myserver__do_thing".to_string(),
            client,
        };
        assert_eq!(wrapper.name(), "mcp__myserver__do_thing");
    }

    #[test]
    fn test_mcp_tool_wrapper_description() {
        let client = Arc::new(McpClient::new("http://localhost:8080"));
        let wrapper = McpToolWrapper {
            tool: make_test_mcp_tool(false),
            prefixed_name: "mcp__s__do_thing".to_string(),
            client,
        };
        assert_eq!(wrapper.description(), "Does a thing");
    }

    #[test]
    fn test_mcp_tool_wrapper_parameters_schema() {
        let client = Arc::new(McpClient::new("http://localhost:8080"));
        let wrapper = McpToolWrapper {
            tool: make_test_mcp_tool(false),
            prefixed_name: "mcp__s__do_thing".to_string(),
            client,
        };
        let schema = wrapper.parameters_schema();
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["input"].is_object());
    }

    #[test]
    fn test_mcp_tool_wrapper_requires_sanitization() {
        let client = Arc::new(McpClient::new("http://localhost:8080"));
        let wrapper = McpToolWrapper {
            tool: make_test_mcp_tool(false),
            prefixed_name: "mcp__s__do_thing".to_string(),
            client,
        };
        assert!(
            wrapper.requires_sanitization(),
            "MCP tools should always require sanitization"
        );
    }

    #[test]
    fn test_mcp_tool_wrapper_approval_destructive() {
        let client = Arc::new(McpClient::new("http://localhost:8080"));
        let wrapper = McpToolWrapper {
            tool: make_test_mcp_tool(true),
            prefixed_name: "mcp__s__do_thing".to_string(),
            client,
        };
        let approval = wrapper.requires_approval(&serde_json::json!({}));
        assert_eq!(approval, ApprovalRequirement::UnlessAutoApproved);
    }

    #[test]
    fn test_mcp_tool_wrapper_approval_non_destructive() {
        let client = Arc::new(McpClient::new("http://localhost:8080"));
        let wrapper = McpToolWrapper {
            tool: make_test_mcp_tool(false),
            prefixed_name: "mcp__s__do_thing".to_string(),
            client,
        };
        let approval = wrapper.requires_approval(&serde_json::json!({}));
        assert_eq!(approval, ApprovalRequirement::Never);
    }
}
