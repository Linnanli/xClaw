//! [`CompositeToolExecutor`] — fan out tool calls across multiple
//! [`ToolExecutor`]s by tool name.
//!
//! ## Why
//!
//! Real hosts (the headless CLI, the ironclaw desktop backend, anything
//! third-party that uses `dasclaw_runtime`) typically draw tools from
//! more than one source at the same time:
//!
//! - a handful of in-process builtins (`echo`, `now`, recorded fixtures)
//! - one or more MCP servers
//! - a future skill registry
//!
//! Without a composite, a host has to choose **one** [`ToolExecutor`] to
//! plug into [`AgentBuilder::tool_executor`](crate::AgentBuilder), which
//! forces an artificial either/or between tool sources. The composite
//! removes that constraint while keeping the `ToolExecutor` contract
//! single-method: dispatch is by **tool name**, decided up front at
//! construction time so the runtime path stays O(1).
//!
//! ## Construction contract
//!
//! Callers pass a list of `(owned_tool_names, executor)` pairs. The
//! composite builds an internal `name → executor` map. Tool names must
//! be **globally unique** across members — duplicates produce a
//! [`CompositeError::DuplicateTool`] at construction so collisions
//! surface at wiring time rather than as silent shadowing at runtime.
//!
//! ## Unknown tools
//!
//! A call with a name that no member owns returns a [`ToolResult`] with
//! `is_error = true` and `content = "unknown tool: <name>"`. This
//! matches both [`McpToolExecutor`](https://docs.rs/dasclaw_mcp) and the
//! CLI's `StaticToolExecutor`, so the model sees a uniform failure shape
//! regardless of which executor would have owned the call.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use dasclaw_core::messages::{ToolCall, ToolResult};
use dasclaw_core::traits::HostError;

use crate::agent::ToolExecutor;

/// Errors raised when assembling a [`CompositeToolExecutor`].
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CompositeError {
    /// Two members claimed ownership of the same tool name. The wiring
    /// caller must rename, qualify, or drop one of them.
    #[error("duplicate tool name across composite members: `{0}`")]
    DuplicateTool(String),
}

/// Routes a [`ToolCall`] to one of several wrapped [`ToolExecutor`]s.
///
/// See the [module docs](crate::composite_executor) for the design
/// rationale and construction contract.
pub struct CompositeToolExecutor {
    routes: HashMap<String, Arc<dyn ToolExecutor>>,
}

impl std::fmt::Debug for CompositeToolExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompositeToolExecutor")
            .field("tools", &self.routes.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl CompositeToolExecutor {
    /// Build a composite from a list of `(owned_tool_names, executor)`
    /// pairs.
    ///
    /// Returns [`CompositeError::DuplicateTool`] if any name appears in
    /// more than one member.
    pub fn new<I>(members: I) -> Result<Self, CompositeError>
    where
        I: IntoIterator<Item = (Vec<String>, Arc<dyn ToolExecutor>)>,
    {
        let mut routes: HashMap<String, Arc<dyn ToolExecutor>> = HashMap::new();
        for (names, exec) in members {
            for name in names {
                if routes.contains_key(&name) {
                    return Err(CompositeError::DuplicateTool(name));
                }
                routes.insert(name, exec.clone());
            }
        }
        Ok(Self { routes })
    }

    /// Number of tool names this composite can route. Exposed for tests
    /// and diagnostics; the agent runtime never reads it.
    pub fn len(&self) -> usize {
        self.routes.len()
    }

    /// Whether the composite owns no tools.
    pub fn is_empty(&self) -> bool {
        self.routes.is_empty()
    }
}

#[async_trait]
impl ToolExecutor for CompositeToolExecutor {
    async fn execute(&self, call: &ToolCall) -> Result<ToolResult, HostError> {
        match self.routes.get(&call.name) {
            Some(exec) => exec.execute(call).await,
            None => Ok(ToolResult {
                tool_call_id: call.id.clone(),
                name: call.name.clone(),
                content: format!("unknown tool: {}", call.name),
                is_error: true,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    /// Trivial in-test executor: every call it accepts echoes a fixed
    /// `marker` so dispatch tests can verify "which member was hit".
    struct MarkerExecutor {
        marker: &'static str,
    }

    #[async_trait]
    impl ToolExecutor for MarkerExecutor {
        async fn execute(&self, call: &ToolCall) -> Result<ToolResult, HostError> {
            Ok(ToolResult {
                tool_call_id: call.id.clone(),
                name: call.name.clone(),
                content: self.marker.to_string(),
                is_error: false,
            })
        }
    }

    fn call(name: &str) -> ToolCall {
        ToolCall {
            id: format!("id-{name}"),
            name: name.to_string(),
            arguments: Value::Null,
            reasoning: None,
        }
    }

    #[tokio::test]
    async fn req_runtime_composite_d1_dispatches_to_owning_member() {
        let alpha = Arc::new(MarkerExecutor { marker: "alpha" }) as Arc<dyn ToolExecutor>;
        let beta = Arc::new(MarkerExecutor { marker: "beta" }) as Arc<dyn ToolExecutor>;
        let composite = CompositeToolExecutor::new([
            (vec!["echo".to_string(), "now".to_string()], alpha),
            (vec!["fs_read".to_string()], beta),
        ])
        .expect("construct");

        let echo_res = composite.execute(&call("echo")).await.expect("echo");
        assert_eq!(echo_res.content, "alpha");
        assert!(!echo_res.is_error);

        let fs_res = composite.execute(&call("fs_read")).await.expect("fs");
        assert_eq!(fs_res.content, "beta");
        assert!(!fs_res.is_error);
    }

    #[tokio::test]
    async fn req_runtime_composite_d2_unknown_tool_returns_is_error() {
        let alpha = Arc::new(MarkerExecutor { marker: "alpha" }) as Arc<dyn ToolExecutor>;
        let composite =
            CompositeToolExecutor::new([(vec!["echo".to_string()], alpha)]).expect("construct");

        let res = composite.execute(&call("ghost")).await.expect("ok wrapper");
        assert!(res.is_error);
        assert_eq!(res.content, "unknown tool: ghost");
        assert_eq!(res.name, "ghost");
        assert_eq!(res.tool_call_id, "id-ghost");
    }

    #[test]
    fn req_runtime_composite_d3_rejects_duplicate_tool_name() {
        let alpha = Arc::new(MarkerExecutor { marker: "alpha" }) as Arc<dyn ToolExecutor>;
        let beta = Arc::new(MarkerExecutor { marker: "beta" }) as Arc<dyn ToolExecutor>;
        let err = CompositeToolExecutor::new([
            (vec!["echo".to_string()], alpha),
            (vec!["echo".to_string()], beta),
        ])
        .expect_err("duplicate");
        assert_eq!(err, CompositeError::DuplicateTool("echo".to_string()));
    }

    #[test]
    fn req_runtime_composite_d4_empty_members_yields_empty_composite() {
        let composite =
            CompositeToolExecutor::new(std::iter::empty::<(Vec<String>, Arc<dyn ToolExecutor>)>())
                .expect("construct");
        assert!(composite.is_empty());
        assert_eq!(composite.len(), 0);
    }
}
