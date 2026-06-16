//! ADR-153 §1.1 B8 (e24): tool-arg schema validation as a pre-execute
//! egress gate.
//!
//! Setup: the scripted responder emits a `read_file` tool call whose
//! arguments are missing the required `path` field (model hallucinated
//! `{"depth": 3}` instead). An `EgressGate` wired with a
//! [`SchemaValidatingToolExecGate`] inspects the serialised arguments
//! at `EgressKind::ToolExecution { tool }` and returns
//! [`EgressDecision::Block`] when the schema check fails. The check
//! itself reuses `dasclaw_safety::Validator::validate_tool_params`
//! for string-content validation (matrix line 138 calls this out by
//! name), layered with a small required-string-field check that the
//! `Validator` itself doesn't perform.
//!
//! Contract verified:
//!   * Block fires at `EgressKind::ToolExecution { tool: "read_file" }`.
//!   * The executor is **never** invoked (`call_count() == 0`).
//!   * The model observes an `Error: …` tool_result containing the
//!     refusal reason and produces a graceful final reply.
//!
//! A positive control (`valid_schema_allows_executor`) pins the
//! contract: a well-formed `{"path":"/tmp/x"}` call passes through
//! and the executor runs exactly once. Without this baseline the
//! Block test could vacuously pass if the gate over-blocked.

use std::sync::Arc;

use async_trait::async_trait;
use dasclaw_core::hooks::{
    AutoApproveGate, EgressDecision, EgressGate, EgressKind, HookBundle, InMemorySecrets,
    NoopSandboxExecutor, RedactionStats,
};
use dasclaw_runtime::{Agent, AgentRunOptions};
use dasclaw_safety::Validator;
use serde_json::json;

#[path = "fixtures/agent_loop_fixtures.rs"]
mod fixtures;

use fixtures::{RecordingToolExecutor, ScriptedResponder, no_tool_defs, text_turn, tool_call_turn};

/// `EgressGate` that validates a single tool's arguments against a
/// minimal "required string fields" schema, plus the project-wide
/// `dasclaw_safety::Validator::validate_tool_params` content checks.
///
/// Scoped to `EgressKind::ToolExecution { tool: target }`; all other
/// kinds (and other tools) pass through. This narrow scope is what
/// e24 asserts: the schema gate must not interfere with `LlmRequest`
/// or `UserDisplay`.
struct SchemaValidatingToolExecGate {
    target_tool: String,
    required_string_fields: Vec<String>,
    refuse_reason_prefix: String,
    validator: Validator,
}

impl SchemaValidatingToolExecGate {
    fn new(
        target_tool: impl Into<String>,
        required_string_fields: impl IntoIterator<Item = impl Into<String>>,
        refuse_reason_prefix: impl Into<String>,
    ) -> Self {
        Self {
            target_tool: target_tool.into(),
            required_string_fields: required_string_fields.into_iter().map(Into::into).collect(),
            refuse_reason_prefix: refuse_reason_prefix.into(),
            validator: Validator::new(),
        }
    }

    /// Returns `Err(structured-reason)` if the payload violates the
    /// schema. Splitting this out keeps `EgressGate::check` async-free
    /// from JSON parsing concerns and makes the failure modes easy
    /// to enumerate in test assertions.
    fn validate(&self, payload: &str) -> Result<(), String> {
        let args: serde_json::Value = serde_json::from_str(payload).map_err(|e| {
            format!(
                "{}: arguments are not valid JSON ({e})",
                self.refuse_reason_prefix
            )
        })?;

        let obj = args.as_object().ok_or_else(|| {
            format!(
                "{}: arguments must be a JSON object, got {}",
                self.refuse_reason_prefix,
                kind_of(&args)
            )
        })?;

        for field in &self.required_string_fields {
            match obj.get(field) {
                None => {
                    return Err(format!(
                        "{}: missing required field `{field}` (expected string)",
                        self.refuse_reason_prefix
                    ));
                }
                Some(v) if !v.is_string() => {
                    return Err(format!(
                        "{}: field `{field}` must be a string, got {}",
                        self.refuse_reason_prefix,
                        kind_of(v)
                    ));
                }
                Some(_) => {}
            }
        }

        // Layer content checks from `dasclaw_safety` on top of the
        // structural ones above. The matrix specifically names
        // `Validator::validate_tool_params` here, so even when the
        // structural check passes we surface the safety verdict.
        let result = self.validator.validate_tool_params(&args);
        if !result.is_valid {
            let detail = result
                .errors
                .iter()
                .map(|e| format!("{}={}", e.field, e.message))
                .collect::<Vec<_>>()
                .join("; ");
            return Err(format!(
                "{}: dasclaw_safety::Validator rejected params [{detail}]",
                self.refuse_reason_prefix
            ));
        }

        Ok(())
    }
}

/// Build the HookBundle used by every test in this file: the
/// `read_file({path:string})` schema gate wired in front of the
/// stock no-op sandbox / in-memory secrets / auto-approve stack.
///
/// Centralising this keeps the three test bodies focused on what
/// they actually assert (block, allow, scope) instead of repeating
/// the same four-field literal.
fn build_hooks() -> HookBundle {
    HookBundle {
        egress: Arc::new(SchemaValidatingToolExecGate::new(
            "read_file",
            ["path"],
            "schema-violation",
        )),
        sandbox: Arc::new(NoopSandboxExecutor),
        secrets: Arc::new(InMemorySecrets::new()),
        approval: Arc::new(AutoApproveGate),
    }
}

fn kind_of(v: &serde_json::Value) -> &'static str {
    match v {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "bool",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

#[async_trait]
impl EgressGate for SchemaValidatingToolExecGate {
    async fn check(&self, kind: &EgressKind, payload: &str) -> EgressDecision {
        let EgressKind::ToolExecution { tool } = kind else {
            return EgressDecision::Allow;
        };
        if tool != &self.target_tool {
            return EgressDecision::Allow;
        }
        match self.validate(payload) {
            Ok(()) => EgressDecision::Allow,
            Err(reason) => EgressDecision::Block {
                reason,
                stats: RedactionStats::default(),
            },
        }
    }
}

#[tokio::test]
async fn req_dasclaw_cli_loop_e24_invalid_schema_blocks_executor() {
    // Turn 1: model hallucinates an arg shape — `depth:3` instead of the
    // required `path:string`.
    // Turn 2: after seeing the `Error: …` tool_result, the model
    //         produces a graceful final reply.
    let responder = ScriptedResponder::with_queue(vec![
        tool_call_turn("read_file", "call_bad_args", json!({ "depth": 3 })),
        text_turn("declined: invalid params for read_file"),
    ]);

    let executor = RecordingToolExecutor::new();
    let calls = executor.calls();

    let agent = Agent::builder()
        .responder(responder)
        .tool_executor(executor)
        .tools(no_tool_defs())
        .hooks(build_hooks())
        .build()
        .expect("agent builds");

    let reply = agent
        .invoke("read the report file", AgentRunOptions::invoke())
        .await
        .expect("loop must finish — Block on a tool call is non-fatal")
        .text;

    assert_eq!(reply, "declined: invalid params for read_file");

    let log = calls.lock().expect("calls log");
    assert_eq!(
        log.len(),
        0,
        "schema-violating tool calls must never reach the executor, got: {log:#?}"
    );
}

#[tokio::test]
async fn req_dasclaw_cli_loop_e24_valid_schema_allows_executor() {
    // Positive control: well-formed args pass the schema gate and the
    // executor runs exactly once. Pins that the gate isn't over-broad.
    let responder = ScriptedResponder::with_queue(vec![
        tool_call_turn(
            "read_file",
            "call_good_args",
            json!({ "path": "/tmp/report.txt" }),
        ),
        text_turn("read complete"),
    ]);

    let executor = RecordingToolExecutor::new().with_reply("read_file", "file body");
    let calls = executor.calls();

    let agent = Agent::builder()
        .responder(responder)
        .tool_executor(executor)
        .tools(no_tool_defs())
        .hooks(build_hooks())
        .build()
        .expect("agent builds");

    let reply = agent
        .invoke("read the report file", AgentRunOptions::invoke())
        .await
        .expect("baseline must run")
        .text;

    assert_eq!(reply, "read complete");

    let log = calls.lock().expect("calls log");
    assert_eq!(
        log.len(),
        1,
        "valid schema must let exactly one execution through, got: {log:#?}"
    );
    assert_eq!(log[0].name, "read_file");
    assert_eq!(log[0].arguments, json!({ "path": "/tmp/report.txt" }));
}

#[tokio::test]
async fn req_dasclaw_cli_loop_e24_schema_gate_scoped_to_target_tool() {
    // The gate is configured for `read_file`. A call to a different
    // tool (`list_files`) — even one whose args would fail the same
    // schema — must Pass through. This pins the scoping contract so a
    // future refactor can't silently make the gate global.
    let responder = ScriptedResponder::with_queue(vec![
        tool_call_turn(
            "list_files",
            "call_list",
            json!({ "depth": 3 }), // would fail `path:string` schema if applied
        ),
        text_turn("listing complete"),
    ]);

    let executor = RecordingToolExecutor::new().with_reply("list_files", "a.txt\nb.txt");
    let calls = executor.calls();

    let agent = Agent::builder()
        .responder(responder)
        .tool_executor(executor)
        .tools(no_tool_defs())
        .hooks(build_hooks())
        .build()
        .expect("agent builds");

    let reply = agent
        .invoke("list files", AgentRunOptions::invoke())
        .await
        .expect("non-target tool must not be gated")
        .text;

    assert_eq!(reply, "listing complete");

    let log = calls.lock().expect("calls log");
    assert_eq!(log.len(), 1, "non-target tool should run, got: {log:#?}");
    assert_eq!(log[0].name, "list_files");
}
