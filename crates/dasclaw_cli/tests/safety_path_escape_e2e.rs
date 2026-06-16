//! ADR-153 §1.1 e11 (A5 — L1 path escape via cap_std).
//!
//! Setup: a temporary workspace root holds a single safe file. A
//! [`CapBoundedFsExecutor`] (defined inline below — intentionally **not**
//! added to `safety_fixtures.rs` because fs semantics are unique to this
//! row) wraps a [`WorkspaceCapability`] and exposes one tool, `fs_read`,
//! which takes a `{ "path": "..." }` argument. The scripted responder
//! drives two scenarios:
//!
//! 1. The model tries to read `../../../../etc/passwd` — cap-std rejects
//!    it as [`WorkspaceCapError::PolicyViolation`]; the executor returns
//!    a `ToolResult` with `is_error = true` and a message containing
//!    `"outside workspace"` (ADR-153 wording) plus the underlying cap-std
//!    detail. The agent loop continues and the model's follow-up turn
//!    becomes the final reply.
//!
//! 2. The model reads `safe.txt` — cap-std resolves the path inside the
//!    workspace; the executor returns the file bytes as `content` and
//!    `is_error = false`. This is the happy-path counterpart that proves
//!    the cap really can serve reads, i.e. the negative case isn't just
//!    failing because every path fails.
//!
//! Contract verified (matches `tests/safety_path_escape_e2e.rs` row in
//! ADR-153 §1.1 line 80):
//!   * Path escape -> `ToolResult { is_error: true, content contains
//!     "outside workspace" }`.
//!   * Loop does not crash; model's follow-up text turn is returned.
//!   * In-bounds read produces the file contents and **no** "outside
//!     workspace" string.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use dasclaw_core::hooks::HookBundle;
use dasclaw_core::messages::{Role, ToolCall, ToolResult};
use dasclaw_core::traits::HostError;
use dasclaw_runtime::{Agent, AgentRunOptions, ToolExecutor};
use dasclaw_workspace_cap::{WorkspaceCapError, WorkspaceCapability};
use tempfile::TempDir;

#[path = "fixtures/safety_fixtures.rs"]
mod safety_fixtures;

use safety_fixtures::{ScriptedResponder, text_turn, tool_call_turn};

/// Inline tool executor that bridges `fs_read({ path })` calls into a
/// capability-bound workspace. Lives in this test file (not the shared
/// fixtures module) because no other e2e row reads files through the
/// host's filesystem capability — keeping it local avoids polluting
/// `safety_fixtures.rs` with single-use scaffolding.
struct CapBoundedFsExecutor {
    cap: WorkspaceCapability,
}

impl CapBoundedFsExecutor {
    fn new(cap: WorkspaceCapability) -> Self {
        Self { cap }
    }
}

#[async_trait]
impl ToolExecutor for CapBoundedFsExecutor {
    async fn execute(&self, call: &ToolCall) -> Result<ToolResult, HostError> {
        // Extract `path` from the call arguments without panicking on
        // malformed input: a missing/non-string `path` is itself a tool
        // error, not a host crash.
        let Some(path) = call.arguments.get("path").and_then(|v| v.as_str()) else {
            return Ok(ToolResult {
                tool_call_id: call.id.clone(),
                name: call.name.clone(),
                content: "error: missing 'path' argument".to_string(),
                is_error: true,
            });
        };

        let content = match self.cap.read(Path::new(path)) {
            Ok(bytes) => ToolResult {
                tool_call_id: call.id.clone(),
                name: call.name.clone(),
                content: String::from_utf8_lossy(&bytes).into_owned(),
                is_error: false,
            },
            // Natural host-layer translation: the ADR wording calls for
            // "outside workspace" in the error message; cap-std's own
            // `Display` impl says "path escapes workspace root". Wrapping
            // it preserves both so future readers can trace which layer
            // tripped without us patching cap-std's error string.
            Err(e @ WorkspaceCapError::PolicyViolation(_)) => ToolResult {
                tool_call_id: call.id.clone(),
                name: call.name.clone(),
                content: format!("error: path outside workspace ({e})"),
                is_error: true,
            },
            Err(e) => ToolResult {
                tool_call_id: call.id.clone(),
                name: call.name.clone(),
                content: format!("error: io ({e})"),
                is_error: true,
            },
        };

        Ok(content)
    }
}

/// Build a tempdir containing `safe.txt` with known contents and return
/// `(tempdir, cap)` — the tempdir handle must be kept alive for the
/// duration of the test so the directory isn't deleted out from under
/// the capability.
fn setup_workspace() -> (TempDir, WorkspaceCapability) {
    let tmp = TempDir::new().expect("create tempdir");
    std::fs::write(tmp.path().join("safe.txt"), b"safe content")
        .expect("seed safe.txt in workspace root");
    let cap = WorkspaceCapability::open(tmp.path()).expect("open workspace capability");
    (tmp, cap)
}

#[tokio::test]
async fn req_dasclaw_cli_safety_e11_path_escape_returns_tool_error() {
    let (_tmp, cap) = setup_workspace();

    let responder = Arc::new(ScriptedResponder::with_queue(vec![
        tool_call_turn(
            "fs_read",
            "c1",
            serde_json::json!({ "path": "../../../../etc/passwd" }),
        ),
        text_turn("declined: path outside workspace"),
    ]));

    let executor = CapBoundedFsExecutor::new(cap);

    let agent = Agent::builder()
        .responder_arc(responder.clone())
        .tool_executor(executor)
        .hooks(HookBundle::noop())
        .build()
        .expect("build agent");

    let reply = agent
        .invoke("read it", AgentRunOptions::invoke())
        .await
        .expect("agent loop completes")
        .text;
    assert_eq!(
        reply, "declined: path outside workspace",
        "loop must complete normally with the model's follow-up turn"
    );

    // Second LLM turn (index 1) is the one that sees the tool_result.
    let snapshot = responder
        .snapshot(1)
        .expect("responder must have been called twice");
    let tool_result_msg = snapshot
        .iter()
        .find(|m| matches!(m.role, Role::Tool))
        .expect("ctx.messages must contain a Tool message after fs_read");

    assert!(
        tool_result_msg.content.contains("outside workspace"),
        "tool_result must surface the ADR-153 boundary phrase; got: {:?}",
        tool_result_msg.content,
    );
    assert!(
        !tool_result_msg.content.contains("safe content"),
        "tool_result must not leak in-workspace file bytes on an escape; got: {:?}",
        tool_result_msg.content,
    );
}

#[tokio::test]
async fn req_dasclaw_cli_safety_e11_path_inside_workspace_succeeds() {
    let (_tmp, cap) = setup_workspace();

    let responder = Arc::new(ScriptedResponder::with_queue(vec![
        tool_call_turn("fs_read", "c1", serde_json::json!({ "path": "safe.txt" })),
        text_turn("file content read"),
    ]));

    let executor = CapBoundedFsExecutor::new(cap);

    let agent = Agent::builder()
        .responder_arc(responder.clone())
        .tool_executor(executor)
        .hooks(HookBundle::noop())
        .build()
        .expect("build agent");

    let reply = agent
        .invoke("read it", AgentRunOptions::invoke())
        .await
        .expect("agent loop completes")
        .text;
    assert_eq!(
        reply, "file content read",
        "happy path must complete with the model's follow-up turn"
    );

    let snapshot = responder
        .snapshot(1)
        .expect("responder must have been called twice");
    let tool_result_msg = snapshot
        .iter()
        .find(|m| matches!(m.role, Role::Tool))
        .expect("ctx.messages must contain a Tool message after fs_read");

    assert!(
        tool_result_msg.content.contains("safe content"),
        "in-bounds read must surface the file contents; got: {:?}",
        tool_result_msg.content,
    );
    assert!(
        !tool_result_msg.content.contains("outside workspace"),
        "happy path must not be tagged as an escape; got: {:?}",
        tool_result_msg.content,
    );
}
