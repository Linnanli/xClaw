//! ADR-153 §1.1 B6 (e22): wire `dasclaw_safety::SafetyLayer::sanitize_for_stash`
//! into the agent loop via the new `ToolOutputSanitizer` seam, and prove
//! that what the **next** LLM iteration sees in its `tool_result` block
//! is the redacted payload — not the raw secret returned by the tool.
//!
//! The test deliberately runs the **full** runtime → safety chain. It
//! does **not** mock `sanitize_for_stash`; it constructs a real
//! `SafetyLayer` with the default leak-detector and asserts the
//! conversation snapshot taken by a capturing responder on iteration 2.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use dasclaw_core::messages::{
    ChatMessage, FinishReason, Role, ToolCall, ToolDefinition, ToolResult,
};
use dasclaw_core::reasoning_ctx::ReasoningContext;
use dasclaw_core::response_types::{RespondOutput, RespondResult, ResponseMetadata, TokenUsage};
use dasclaw_core::traits::HostError;
use dasclaw_runtime::{Agent, AgentResponder, ToolExecutor, ToolOutputSanitizer};
use dasclaw_safety::{SafetyConfig, SafetyLayer};
use serde_json::json;

/// Adapter wiring `SafetyLayer::sanitize_for_stash` (redact-only, no
/// length cap) into the `ToolOutputSanitizer` seam. Lives in the test
/// because `dasclaw_runtime` stays safety-stack-agnostic by design.
struct SafetyStashSanitizer {
    layer: Arc<SafetyLayer>,
}

impl ToolOutputSanitizer for SafetyStashSanitizer {
    fn sanitize(&self, tool_name: &str, content: &str) -> String {
        self.layer.sanitize_for_stash(tool_name, content).content
    }
}

/// Responder that **captures** `ctx.messages.clone()` on every call
/// before popping a scripted reply. Lets the test assert what the LLM
/// saw on iteration 2 (the call that observes the post-tool-result
/// snapshot), without having to expose internal loop state.
struct CapturingResponder {
    script: Mutex<Vec<RespondOutput>>,
    snapshots: Mutex<Vec<Vec<ChatMessage>>>,
}

impl CapturingResponder {
    fn new(script: Vec<RespondOutput>) -> Self {
        Self {
            script: Mutex::new(script),
            snapshots: Mutex::new(Vec::new()),
        }
    }

    fn snapshots(&self) -> Vec<Vec<ChatMessage>> {
        self.snapshots.lock().expect("snapshots mutex").clone()
    }
}

#[async_trait]
impl AgentResponder for CapturingResponder {
    async fn respond(&self, ctx: &mut ReasoningContext) -> Result<RespondOutput, HostError> {
        self.snapshots
            .lock()
            .expect("snapshots mutex")
            .push(ctx.messages.clone());
        let mut s = self.script.lock().expect("script mutex");
        if s.is_empty() {
            return Err("script exhausted".into());
        }
        Ok(s.remove(0))
    }
}

/// Minimal map-based tool executor used only for e22. Returns the
/// configured reply for a tool name, or a generic "ok" body.
struct ReplyingToolExecutor {
    replies: HashMap<String, String>,
}

impl ReplyingToolExecutor {
    fn new() -> Self {
        Self {
            replies: HashMap::new(),
        }
    }

    fn with_reply(mut self, name: &str, body: impl Into<String>) -> Self {
        self.replies.insert(name.to_string(), body.into());
        self
    }
}

#[async_trait]
impl ToolExecutor for ReplyingToolExecutor {
    async fn execute(&self, call: &ToolCall) -> Result<ToolResult, HostError> {
        let content = self
            .replies
            .get(&call.name)
            .cloned()
            .unwrap_or_else(|| "ok".to_string());
        Ok(ToolResult {
            tool_call_id: call.id.clone(),
            name: call.name.clone(),
            content,
            is_error: false,
        })
    }
}

fn text_turn(text: &str) -> RespondOutput {
    RespondOutput {
        result: RespondResult::Text(text.to_string()),
        usage: TokenUsage::default(),
        finish_reason: FinishReason::Stop,
        metadata: ResponseMetadata::default(),
    }
}

fn tool_call_turn(name: &str, id: &str, args: serde_json::Value) -> RespondOutput {
    RespondOutput {
        result: RespondResult::ToolCalls {
            content: None,
            tool_calls: vec![ToolCall {
                id: id.to_string(),
                name: name.to_string(),
                arguments: args,
                reasoning: None,
            }],
        },
        usage: TokenUsage::default(),
        finish_reason: FinishReason::ToolUse,
        metadata: ResponseMetadata::default(),
    }
}

#[tokio::test]
async fn req_dasclaw_cli_loop_e22_tool_output_redacted_before_next_llm_call() {
    // A Bearer-style token (matches the `bearer_token` LeakPattern's
    // regex `Bearer\s+[a-zA-Z0-9_-]{20,}`). That pattern uses
    // `LeakAction::Redact`, so `sanitize_for_stash` rewrites the secret
    // to `[REDACTED]` while keeping the surrounding context intact.
    let secret = "Bearer abcdef0123456789ABCDEFXYZ";
    let tool_payload = format!("found credential: {secret} (please rotate)");

    let script = vec![
        tool_call_turn("read_file", "call_read_1", json!({ "path": "secrets.txt" })),
        text_turn("done — credential surfaced and routed for rotation"),
    ];
    let responder = Arc::new(CapturingResponder::new(script));

    let executor = ReplyingToolExecutor::new().with_reply("read_file", tool_payload.clone());

    let safety = Arc::new(SafetyLayer::new(&SafetyConfig {
        max_output_length: 65_536,
        injection_check_enabled: false,
    }));
    let sanitizer = SafetyStashSanitizer {
        layer: Arc::clone(&safety),
    };

    let agent = Agent::builder()
        .responder_arc(responder.clone())
        .tool_executor(executor)
        .tool_output_sanitizer(sanitizer)
        .tools(Vec::<ToolDefinition>::new())
        .build()
        .expect("agent builds");

    let reply = agent
        .run("scan for leaked credentials")
        .await
        .expect("loop must finish — redaction is non-fatal");
    assert_eq!(reply, "done — credential surfaced and routed for rotation");

    let snapshots = responder.snapshots();
    assert_eq!(
        snapshots.len(),
        2,
        "expected exactly two LLM calls (tool-call turn, then final-text turn), got {}",
        snapshots.len()
    );

    // Iteration 2 is the LLM call that observes the tool_result message
    // produced after the tool ran. That message must carry the redacted
    // payload, never the raw secret — that's the whole point of B6.
    let second_view = &snapshots[1];
    let tool_result_msg = second_view
        .iter()
        .rev()
        .find(|m| matches!(m.role, Role::Tool))
        .expect("iteration 2 must see at least one tool_result message");
    let body = &tool_result_msg.content;

    assert!(
        !body.contains(secret),
        "tool_result body must NOT contain the raw secret; got body = {body:?}"
    );
    assert!(
        body.contains("[REDACTED]"),
        "tool_result body must contain `[REDACTED]` from sanitize_for_stash; got body = {body:?}"
    );
    assert!(
        body.contains("please rotate"),
        "non-secret context must survive redaction; got body = {body:?}"
    );
}

#[tokio::test]
async fn req_dasclaw_cli_loop_e22_no_sanitizer_forwards_verbatim_baseline() {
    // Negative control: without `tool_output_sanitizer`, the loop must
    // forward tool output verbatim. This pins the opt-in contract so a
    // future refactor can't silently start redacting without callers
    // asking for it.
    let secret_like = "Bearer abcdef0123456789ABCDEFXYZ";
    let tool_payload = format!("verbatim: {secret_like}");

    let script = vec![
        tool_call_turn(
            "read_file",
            "call_read_1",
            json!({ "path": "verbatim.txt" }),
        ),
        text_turn("ok"),
    ];
    let responder = Arc::new(CapturingResponder::new(script));
    let executor = ReplyingToolExecutor::new().with_reply("read_file", tool_payload.clone());

    let agent = Agent::builder()
        .responder_arc(responder.clone())
        .tool_executor(executor)
        .tools(Vec::<ToolDefinition>::new())
        .build()
        .expect("agent builds");

    agent.run("baseline").await.expect("baseline must run");

    let snapshots = responder.snapshots();
    assert_eq!(snapshots.len(), 2);
    let second_view = &snapshots[1];
    let tool_result_msg = second_view
        .iter()
        .rev()
        .find(|m| matches!(m.role, Role::Tool))
        .expect("iteration 2 must see a tool_result");
    assert_eq!(
        tool_result_msg.content, tool_payload,
        "without a sanitizer, the loop must forward tool output byte-for-byte"
    );
}
