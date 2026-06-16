//! Issue #911 (B5 — claw-code interop demo on the GUI/headless path).
//!
//! Wires three `dasclaw_*` crates together to prove the three-lineage
//! fusion called out in the GUI readiness assessment:
//!
//! * [`dasclaw_runtime::Agent`] drives a scripted four-turn agent loop.
//! * [`dasclaw_hooks::BashValidationHook`] (which itself depends on
//!   [`dasclaw_bash_validation::validate_command`]) sits on the egress
//!   gate and rejects `rm -rf /` before the executor sees it.
//! * [`dasclaw_apply_patch::parse_patch`] and
//!   [`dasclaw_apply_patch::apply`] write the assistant's patch onto
//!   the demo workspace.
//!
//! See `docs/INTEROP_DASCLAW.md` for the claw-code → dasclaw_* concept
//! map (relocated from `claw-code/` because that path is a git
//! submodule in this workspace and is not writable from the parent
//! repo).
//!
//! All public items live at the top of this file so the matching e2e
//! test (`tests/claw_code_interop_e2e.rs`) can `#[path]`-include them
//! without dragging the dev-only deps into `src/`.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use dasclaw_apply_patch::{ApplyOptions, apply, parse_patch};
use dasclaw_bash_validation::{
    PermissionMode as BvPermissionMode, ValidationResult as BvValidationResult,
    validate_command as bv_validate_command,
};
use dasclaw_core::hooks::HookBundle;
use dasclaw_core::messages::{FinishReason, ToolCall, ToolDefinition, ToolResult};
use dasclaw_core::permissions::PermissionMode;
use dasclaw_core::reasoning_ctx::ReasoningContext;
use dasclaw_core::response_types::{RespondOutput, RespondResult, ResponseMetadata, TokenUsage};
use dasclaw_core::traits::HostError;
use dasclaw_hooks::BashValidationHook;
use dasclaw_runtime::{Agent, AgentResponder, AgentRunOptions, ToolExecutor};
use serde_json::json;

/// A single entry in the demo's audit log.
///
/// The gate (decision lane) and the executor (execution lane) both push
/// into the same `Vec` so the timeline reflects the order the agent
/// loop dispatched calls.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEntry {
    pub tool: String,
    pub outcome: AuditOutcome,
}

/// What happened to a given tool call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditOutcome {
    /// Egress gate rejected the call; the executor never ran for it.
    /// Carries the gate's human-readable reason string.
    BlockedByGate(String),
    /// The executor ran and produced this textual result.
    ExecutorOk(String),
}

/// Final result returned by [`run_demo`].
#[derive(Debug, Clone)]
pub struct DemoOutcome {
    /// Final assistant text from the agent loop — the literal `done`
    /// produced by [`FakeResponder`] on turn 4.
    pub final_text: String,
    /// Combined timeline of every gate decision and every executor
    /// invocation, in dispatch order.
    pub audit_log: Vec<AuditEntry>,
    /// Contents of `<workspace>/hello.txt` after the demo finishes,
    /// used by the e2e test to prove the patch landed.
    pub hello_txt: String,
}

/// Construct a `HostError` from a stringy message without panicking.
fn host_err<M: Into<String>>(msg: M) -> HostError {
    Box::<dyn std::error::Error + Send + Sync>::from(msg.into())
}

/// Four-turn scripted "model":
///
/// 1. call `bash` with `rm -rf /` (must be blocked by the gate);
/// 2. call `bash` with `ls` (must be allowed);
/// 3. call `apply_patch` rewriting `hello.txt`;
/// 4. emit final text `done`.
struct FakeResponder {
    turns: Mutex<Vec<RespondOutput>>,
}

impl FakeResponder {
    fn new() -> Self {
        let tool_turn = |id: &str, name: &str, args: serde_json::Value| RespondOutput {
            result: RespondResult::ToolCalls {
                tool_calls: vec![ToolCall {
                    id: id.into(),
                    name: name.into(),
                    arguments: args,
                    reasoning: None,
                }],
                content: None,
            },
            usage: TokenUsage::default(),
            finish_reason: FinishReason::ToolUse,
            metadata: ResponseMetadata::default(),
        };
        let patch_body = "*** Begin Patch\n\
                          *** Update File: hello.txt\n\
                          @@\n\
                          -hello\n\
                          +world\n\
                          *** End Patch\n";
        let turns = vec![
            tool_turn("c1", "bash", json!({ "command": "rm -rf /" })),
            tool_turn("c2", "bash", json!({ "command": "ls" })),
            tool_turn("c3", "apply_patch", json!({ "input": patch_body })),
            RespondOutput {
                result: RespondResult::Text("done".into()),
                usage: TokenUsage::default(),
                finish_reason: FinishReason::Stop,
                metadata: ResponseMetadata::default(),
            },
        ];
        Self {
            turns: Mutex::new(turns),
        }
    }
}

#[async_trait]
impl AgentResponder for FakeResponder {
    async fn respond(
        &self,
        _ctx: &mut ReasoningContext,
    ) -> std::result::Result<RespondOutput, HostError> {
        let mut q = self
            .turns
            .lock()
            .map_err(|e| host_err(format!("FakeResponder lock poisoned: {e}")))?;
        if q.is_empty() {
            return Err(host_err(
                "FakeResponder ran out of turns (loop iterated beyond the 4-turn script)",
            ));
        }
        Ok(q.remove(0))
    }
}

/// Egress gate wrapper that delegates to a real
/// [`BashValidationHook`] and records every `Block` decision into a
/// shared audit log.
///
/// We need our own wrapper because once the agent loop converts a
/// `Block` into a tool error `ToolResult`, the original reason string
/// is otherwise dropped before the host sees it.
struct RecordingGate {
    inner: BashValidationHook,
    log: Arc<Mutex<Vec<AuditEntry>>>,
}

#[async_trait]
impl dasclaw_core::EgressGate for RecordingGate {
    async fn check(
        &self,
        kind: &dasclaw_core::EgressKind,
        payload: &str,
    ) -> dasclaw_core::EgressDecision {
        let decision = self.inner.check(kind, payload).await;
        if let dasclaw_core::EgressKind::ToolExecution { tool } = kind
            && let dasclaw_core::EgressDecision::Block { reason, .. } = &decision
            && let Ok(mut log) = self.log.lock()
        {
            log.push(AuditEntry {
                tool: tool.clone(),
                outcome: AuditOutcome::BlockedByGate(reason.clone()),
            });
        }
        decision
    }
}

/// Stub executor handling exactly two tools:
///
/// * `bash` — never spawns a real shell; performs a defense-in-depth
///   call into [`dasclaw_bash_validation::validate_command`] on top of
///   the gate, then returns a canned string.
/// * `apply_patch` — drives [`parse_patch`] + [`apply`] with
///   `base_dir = workspace`.
struct InteropExecutor {
    workspace: PathBuf,
    log: Arc<Mutex<Vec<AuditEntry>>>,
}

#[async_trait]
impl ToolExecutor for InteropExecutor {
    async fn execute(&self, call: &ToolCall) -> std::result::Result<ToolResult, HostError> {
        match call.name.as_str() {
            "bash" => self.exec_bash(call),
            "apply_patch" => self.exec_apply_patch(call),
            other => Err(host_err(format!(
                "InteropExecutor: unsupported tool {other}"
            ))),
        }
    }
}

impl InteropExecutor {
    fn exec_bash(&self, call: &ToolCall) -> std::result::Result<ToolResult, HostError> {
        let cmd = call
            .arguments
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| host_err("bash tool call missing `command` arg"))?;
        // Defense-in-depth pre-check: directly invoke
        // dasclaw_bash_validation::validate_command before pretending
        // to run anything. By the time we reach the executor the gate
        // already approved, so Allow is the only branch we expect; any
        // other outcome means the two layers disagree and we surface
        // that back to the model as a tool error rather than panicking.
        match bv_validate_command(cmd, BvPermissionMode::ReadOnly, &self.workspace) {
            BvValidationResult::Allow => {}
            BvValidationResult::Warn { message } => {
                return Ok(error_result(
                    call,
                    format!("defense-in-depth disagreement (Warn): {message}"),
                ));
            }
            BvValidationResult::Block { reason } => {
                return Ok(error_result(
                    call,
                    format!("defense-in-depth Block (gate missed it): {reason}"),
                ));
            }
        }
        let content = format!("(stub) ran: {cmd}");
        if let Ok(mut log) = self.log.lock() {
            log.push(AuditEntry {
                tool: "bash".into(),
                outcome: AuditOutcome::ExecutorOk(content.clone()),
            });
        }
        Ok(ToolResult {
            tool_call_id: call.id.clone(),
            name: call.name.clone(),
            content,
            is_error: false,
        })
    }

    fn exec_apply_patch(&self, call: &ToolCall) -> std::result::Result<ToolResult, HostError> {
        let input = call
            .arguments
            .get("input")
            .and_then(|v| v.as_str())
            .ok_or_else(|| host_err("apply_patch tool call missing `input` arg"))?;
        let parsed =
            parse_patch(input).map_err(|e| host_err(format!("parse_patch failed: {e}")))?;
        let opts = ApplyOptions {
            base_dir: Some(self.workspace.clone()),
        };
        let report = apply(&parsed, &opts).map_err(|e| host_err(format!("apply failed: {e}")))?;
        let content = format!(
            "(stub) applied patch: {} hunk(s), {} added, {} updated, {} deleted",
            report.hunks_applied,
            report.files_added.len(),
            report.files_updated.len(),
            report.files_deleted.len(),
        );
        if let Ok(mut log) = self.log.lock() {
            log.push(AuditEntry {
                tool: "apply_patch".into(),
                outcome: AuditOutcome::ExecutorOk(content.clone()),
            });
        }
        Ok(ToolResult {
            tool_call_id: call.id.clone(),
            name: call.name.clone(),
            content,
            is_error: false,
        })
    }
}

fn error_result(call: &ToolCall, msg: String) -> ToolResult {
    ToolResult {
        tool_call_id: call.id.clone(),
        name: call.name.clone(),
        content: msg,
        is_error: true,
    }
}

/// Build the [`HookBundle`] that wires our [`RecordingGate`] (on top of
/// a real [`BashValidationHook`] in `ReadOnly` mode) as the egress
/// lane, leaving the other three lanes at their no-op defaults.
fn build_hooks(workspace: &Path, log: Arc<Mutex<Vec<AuditEntry>>>) -> HookBundle {
    let inner = BashValidationHook::new(PermissionMode::ReadOnly, workspace.to_path_buf());
    let gate = Arc::new(RecordingGate { inner, log });
    HookBundle {
        egress: gate,
        ..HookBundle::noop()
    }
}

fn tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "bash".into(),
            description: "Stubbed bash — never spawns a real shell.".into(),
            parameters: json!({
                "type": "object",
                "properties": { "command": { "type": "string" } },
                "required": ["command"]
            }),
        },
        ToolDefinition {
            name: "apply_patch".into(),
            description: "Apply a *** Begin Patch / *** End Patch hunk.".into(),
            parameters: json!({
                "type": "object",
                "properties": { "input": { "type": "string" } },
                "required": ["input"]
            }),
        },
    ]
}

/// Run the demo end-to-end inside `workspace`.
///
/// The caller passes a freshly created tempdir; this function seeds
/// `<workspace>/hello.txt`, drives the agent loop, and returns the
/// combined timeline.
pub async fn run_demo(workspace: &Path) -> Result<DemoOutcome> {
    std::fs::write(workspace.join("hello.txt"), "hello\n")
        .with_context(|| format!("seeding hello.txt under {}", workspace.display()))?;
    let log: Arc<Mutex<Vec<AuditEntry>>> = Arc::new(Mutex::new(Vec::new()));
    let executor = InteropExecutor {
        workspace: workspace.to_path_buf(),
        log: Arc::clone(&log),
    };
    let hooks = build_hooks(workspace, Arc::clone(&log));
    let agent = Agent::builder()
        .responder(FakeResponder::new())
        .tool_executor(executor)
        .tools(tool_definitions())
        .hooks(hooks)
        .build()
        .map_err(|e| anyhow!("AgentBuilder::build failed: {e}"))?;
    let final_text = agent
        .invoke(
            "Use the tools to write 'world' into hello.txt, then say 'done'.",
            AgentRunOptions::invoke(),
        )
        .await
        .map_err(|e| anyhow!("Agent::invoke failed: {e}"))?
        .text;
    let hello_txt = std::fs::read_to_string(workspace.join("hello.txt"))
        .with_context(|| format!("reading hello.txt under {}", workspace.display()))?;
    let audit_log = log
        .lock()
        .map_err(|e| anyhow!("audit log lock poisoned: {e}"))?
        .clone();
    Ok(DemoOutcome {
        final_text,
        audit_log,
        hello_txt,
    })
}

// Gated off the test build so the e2e test (`tests/claw_code_interop_e2e.rs`)
// can `#[path]`-include this file without dragging in a second `main` —
// `cargo build --example` leaves `cfg(test)` unset and gets the real binary.
#[cfg(not(test))]
#[tokio::main]
async fn main() -> Result<()> {
    let tmp = tempfile::tempdir().context("creating tempdir for demo workspace")?;
    let outcome = run_demo(tmp.path()).await?;
    println!("final assistant text: {}", outcome.final_text);
    println!("hello.txt after demo: {}", outcome.hello_txt.trim_end());
    println!("audit timeline:");
    for (i, entry) in outcome.audit_log.iter().enumerate() {
        match &entry.outcome {
            AuditOutcome::BlockedByGate(reason) => {
                println!("  [{i}] {} -> BLOCKED ({reason})", entry.tool);
            }
            AuditOutcome::ExecutorOk(text) => {
                println!("  [{i}] {} -> {text}", entry.tool);
            }
        }
    }
    Ok(())
}
