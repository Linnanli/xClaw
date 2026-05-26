//! `--wasm-tool` flag wiring (ADR-153 §1.1 A7 / #867).
//!
//! Loads a `wasm32-wasip2` component from disk and wraps it in a
//! [`ToolExecutor`] suitable for the [`crate::run_with_tools`] /
//! [`crate::run_with_tools_and_hooks`] entry points.
//!
//! Capability policy: the wrapper is constructed with
//! [`Capabilities::default()`] (no `http`, no workspace writes, no
//! secrets). This matches the W6.6c e13 default-deny posture — a wasm
//! tool that tries to call an ungated host import sees the gate string
//! (`"HTTP capability not granted"`, etc.) come back as
//! `ToolResult { is_error: true, .. }` rather than crashing the host.
//! Granting capabilities to a CLI-loaded wasm tool is out of scope for
//! #867 and tracked as #869.
//!
//! This module is only compiled with the `wasm-tools` feature; see
//! `Cargo.toml` for the binary-size rationale.

use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use async_trait::async_trait;
use dasclaw_core::messages::{ToolCall, ToolDefinition, ToolResult};
use dasclaw_core::traits::HostError;
use dasclaw_runtime::Tool;
use dasclaw_runtime::ToolExecutor;
use dasclaw_runtime::context::JobContext;
use dasclaw_wasm_tools::{Capabilities, WasmRuntimeConfig, WasmToolRuntime, WasmToolWrapper};
use serde_json::json;

/// A loaded wasm tool ready to plug into [`crate::run_with_tools`].
///
/// Exposes the [`ToolDefinition`] separately so the caller can merge it
/// with builtin/MCP definitions in `main.rs` without re-deriving it.
pub struct LoadedWasmTool {
    pub definition: ToolDefinition,
    pub executor: WasmToolExecutor,
}

/// Read a wasm component from `path` and prepare it for execution under
/// the headless capability-default-deny policy.
///
/// The tool's LLM-facing name is the file stem (filename without the
/// `.wasm` extension). The advertised schema is an empty object — wasm
/// tools today don't ship their own JSON schema, so we hand the model
/// the most permissive shape; refining this is a follow-up tracked in
/// #869.
pub async fn load_wasm_tool(path: &Path) -> Result<LoadedWasmTool> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("reading wasm component from {}", path.display()))?;

    let tool_name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow::anyhow!("wasm path has no usable file stem: {}", path.display()))?
        .to_string();

    // Default config is too tight for the W6.6c fixture (~17 pages ≈ 1.1 MB
    // > 1 MB cap). Lift the per-tool memory limit to 4 MB — same value the
    // e13 fixture uses — without touching the global default. Anything
    // larger should land via a future `--wasm-mem-limit` flag (#869).
    let mut config = WasmRuntimeConfig::for_testing();
    config.default_limits = config.default_limits.with_memory(4 * 1024 * 1024);

    let runtime =
        Arc::new(WasmToolRuntime::new(config).context("initializing wasm component runtime")?);
    let prepared = runtime
        .prepare(&tool_name, &bytes, None)
        .await
        .with_context(|| format!("preparing wasm tool `{tool_name}`"))?;
    let wrapper = Arc::new(WasmToolWrapper::new(
        Arc::clone(&runtime),
        prepared,
        Capabilities::default(),
    ));

    Ok(LoadedWasmTool {
        definition: ToolDefinition {
            name: tool_name.clone(),
            description: format!("Sandboxed wasm tool loaded from {}", path.display()),
            parameters: json!({ "type": "object", "properties": {} }),
        },
        executor: WasmToolExecutor { tool_name, wrapper },
    })
}

/// [`ToolExecutor`] that forwards a single named call into a
/// [`WasmToolWrapper`] and maps wrapper errors (including capability
/// gate refusals) onto reported tool errors rather than `HostError`s.
///
/// Mirrors the routing shape used by the e13 fixture executor: any
/// wrapper failure becomes `ToolResult { is_error: true, content: <msg> }`,
/// matching the contract `run_with_tools_and_hooks` expects.
pub struct WasmToolExecutor {
    tool_name: String,
    wrapper: Arc<WasmToolWrapper>,
}

#[async_trait]
impl ToolExecutor for WasmToolExecutor {
    async fn execute(&self, call: &ToolCall) -> Result<ToolResult, HostError> {
        if call.name != self.tool_name {
            return Err(HostError::from(format!(
                "unexpected tool call: {} (wasm executor only routes {})",
                call.name, self.tool_name
            )));
        }

        let mut ctx = JobContext::new("cli-wasm", "wasm tool invocation");
        let outcome = self.wrapper.execute(call.arguments.clone(), &mut ctx).await;
        let (is_error, content) = match outcome {
            Ok(output) => (false, serde_json::to_string(&output).unwrap_or_default()),
            Err(err) => (true, err.to_string()),
        };

        Ok(ToolResult {
            tool_call_id: call.id.clone(),
            name: call.name.clone(),
            content,
            is_error,
        })
    }
}
