//! ADR-153 §1.1 case **e16 (A10)** — L8 网络审计。
//!
//! Contract:
//! - 一轮带工具的对话中，`HookBundle.egress` 会被分别以 `LlmRequest` /
//!   `ToolExecution { tool }` / `UserDisplay` 三种 [`EgressKind`] 触发
//!   至少一次（W6.1 接线分别在 `dasclaw_core::agentic_loop` 和
//!   `dasclaw_runtime::agent` 中落地）。
//! - 一个 audit-recording gate 可以把这三类 egress 全部记录下来，
//!   且记录里 payload **已 redact**，即 audit log 本身不会成为新的
//!   信息泄露 sink。
//!
//! 这与 e7（A1，单点 LlmRequest 阻断）、e8（A2，单点 ToolExecution 阻断）、
//! e15（A8，单点 UserDisplay 重写）互补：那三例验证单一 kind 路径，
//! 本例验证 audit gate **跨三类 kind** 的端到端可观测性。

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use dasclaw_cli::run_with_tools_and_hooks;
use dasclaw_core::hooks::{
    AutoApproveGate, EgressDecision, EgressGate, EgressKind, HookBundle, InMemorySecrets,
    NoopSandboxExecutor,
};
use dasclaw_core::messages::ToolDefinition;

#[path = "fixtures/safety_fixtures.rs"]
mod safety_fixtures;

use safety_fixtures::{RecordingToolExecutor, ScriptedResponder, text_turn, tool_call_turn};

/// Trigger token that must never appear in any audit record.
///
/// Chosen to be distinctive so an accidental partial match (e.g. only
/// "SK_LIVE") would still fail the assertion.
const TRIGGER: &str = "SK_LIVE_TRIGGER_AUDIT_E16";

/// Single audit record: `(kind, post-decision payload)`.
type AuditRecord = (EgressKind, String);
type AuditLog = Arc<Mutex<Vec<AuditRecord>>>;

/// Audit-safe `EgressGate`:
///
/// - If the payload contains [`TRIGGER`], return [`EgressDecision::Redact`]
///   with `[REDACTED]` substituted in.
/// - Otherwise [`EgressDecision::Allow`].
///
/// **Audit invariant**: only the post-decision (sanitized) payload is
/// stored in the call log. The raw `payload` argument is never persisted,
/// so the audit log itself cannot leak the trigger.
struct AuditRedactingGate {
    records: AuditLog,
}

impl AuditRedactingGate {
    fn new() -> (Arc<Self>, AuditLog) {
        let records: AuditLog = Arc::new(Mutex::new(Vec::new()));
        let gate = Arc::new(Self {
            records: Arc::clone(&records),
        });
        (gate, records)
    }
}

#[async_trait]
impl EgressGate for AuditRedactingGate {
    async fn check(&self, kind: &EgressKind, payload: &str) -> EgressDecision {
        let (decision, audit_payload) = if payload.contains(TRIGGER) {
            let sanitized = payload.replace(TRIGGER, "[REDACTED]");
            (
                EgressDecision::Redact {
                    sanitized: sanitized.clone(),
                    stats: Default::default(),
                },
                sanitized,
            )
        } else {
            (EgressDecision::Allow, payload.to_string())
        };

        if let Ok(mut log) = self.records.lock() {
            log.push((kind.clone(), audit_payload));
        }
        decision
    }
}

fn tool_def() -> ToolDefinition {
    ToolDefinition {
        name: "do_thing".into(),
        description: "test tool for A10 audit coverage".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "key": { "type": "string" }
            }
        }),
    }
}

#[tokio::test]
async fn req_dasclaw_cli_safety_a10_egress_audit_log_redacts_all_kinds() {
    // user prompt embeds the trigger → exercises the LlmRequest path.
    let user_prompt = format!("please call do_thing with {TRIGGER}");

    // Turn 1: model emits a tool call whose arguments contain the
    //         trigger → exercises the ToolExecution path.
    // Turn 2: model emits a plain-text reply that echoes the trigger
    //         → exercises the UserDisplay path.
    let responder = ScriptedResponder::with_queue(vec![
        tool_call_turn("do_thing", "call_1", serde_json::json!({ "key": TRIGGER })),
        text_turn(format!("done observing {TRIGGER}")),
    ]);

    let executor = RecordingToolExecutor::with_content(format!("observed {TRIGGER}"));

    let (gate, records) = AuditRedactingGate::new();
    let hooks = HookBundle {
        egress: gate,
        sandbox: Arc::new(NoopSandboxExecutor),
        secrets: Arc::new(InMemorySecrets::new()),
        approval: Arc::new(AutoApproveGate),
    };

    let reply = run_with_tools_and_hooks(
        responder,
        executor,
        vec![tool_def()],
        hooks,
        "system",
        user_prompt.as_str(),
    )
    .await
    .expect("Redact is non-blocking; agent run must succeed");

    // The final reply that surfaces to the caller has flowed through the
    // UserDisplay gate, so the trigger must already be redacted.
    assert!(
        !reply.contains(TRIGGER),
        "rendered reply must be redacted, got: {reply}"
    );

    let log = records.lock().expect("audit log lock");
    assert!(
        !log.is_empty(),
        "audit gate must have observed at least one egress event"
    );

    let mut saw_llm_request = false;
    let mut saw_tool_execution = false;
    let mut saw_user_display = false;

    for (kind, audit_payload) in log.iter() {
        // Core A10 invariant: no audit record may carry the raw trigger.
        assert!(
            !audit_payload.contains(TRIGGER),
            "audit record payload leaked the trigger: kind={kind:?} payload={audit_payload}"
        );

        match kind {
            EgressKind::LlmRequest => saw_llm_request = true,
            EgressKind::ToolExecution { tool } => {
                assert_eq!(
                    tool, "do_thing",
                    "ToolExecution kind must carry the registered tool name"
                );
                saw_tool_execution = true;
            }
            EgressKind::UserDisplay => saw_user_display = true,
            _ => {}
        }
    }

    assert!(
        saw_llm_request,
        "expected at least one EgressKind::LlmRequest audit record, observed: {log:?}"
    );
    assert!(
        saw_tool_execution,
        "expected at least one EgressKind::ToolExecution audit record, observed: {log:?}"
    );
    assert!(
        saw_user_display,
        "expected at least one EgressKind::UserDisplay audit record, observed: {log:?}"
    );
}
