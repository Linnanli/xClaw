//! GUI approval loop primitives (issue #910, GUI blocker B4).
//!
//! The headless agent (see [`crate::agent`]) previously refused every
//! approval prompt with a fail-stop [`crate::AgentError::ApprovalRequested`].
//! That contract is fine for fully automated agents but blocks any GUI
//! that wants a "user-in-the-loop" workflow for dangerous tool calls.
//!
//! This module supplies the pieces the GUI loop needs without touching
//! the `dasclaw_core` `LoopDelegate` / `LoopOutcome` public contract:
//!
//! - [`ApprovalPolicy`] — pluggable rule that decides whether a given
//!   [`dasclaw_core::messages::ToolCall`] needs human approval. Default
//!   is [`NoApprovalPolicy`], which preserves the pre-B4 zero-event
//!   behaviour for every existing caller.
//! - [`ApprovalRequest`] — the policy's verdict carrying the
//!   user-facing description, sanitised parameters and the
//!   `allow_always` flag.
//! - [`ApprovalDecision`] — the GUI's reply: `Approve`, `Reject`, or
//!   `ApproveAlways`.
//! - [`ApprovalDispatchError`] — surfaced by
//!   [`crate::Agent::respond_to_approval`] when the supplied request id
//!   is unknown or its channel has been dropped.
//!
//! The runtime owns one [`ApprovalInbox`] per [`crate::Agent`]; the
//! `HeadlessDelegate` inserts a one-shot sender when it emits
//! [`crate::AgentEvent::ApprovalNeeded`] and the GUI calls
//! [`crate::Agent::respond_to_approval`] to resolve it.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use async_trait::async_trait;
use dasclaw_core::messages::ToolCall;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use crate::AgentEvent;

/// What the GUI tells the runtime once the user (dis)approves a call.
///
/// Serialised as adjacent-tagged JSON (`{"kind":"approve"}` /
/// `{"kind":"reject","data":{"reason":"…"}}` /
/// `{"kind":"approve_always"}`) so a TypeScript discriminated union
/// renders directly from [`serde_json::to_string`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum ApprovalDecision {
    /// Run this tool call exactly once and continue the loop.
    Approve,
    /// Refuse this tool call. The optional `reason` is forwarded both to
    /// the LLM (as a tool-error `tool_result`) and to the
    /// [`crate::AgentError::ApprovalRejected`] surface so the GUI can
    /// render its own copy.
    Reject {
        /// Free-form rationale shown to the LLM and the GUI.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    /// Run this call and remember the decision for future calls of the
    /// same tool. The policy decides what "remember" means in practice;
    /// the runtime treats it identically to [`ApprovalDecision::Approve`]
    /// for the in-flight call.
    ApproveAlways,
}

/// Surface for [`crate::Agent::respond_to_approval`].
///
/// Both variants are recoverable from the GUI's perspective: an
/// [`ApprovalDispatchError::Unknown`] typically means the user clicked
/// twice, and [`ApprovalDispatchError::Closed`] means the loop already
/// gave up on the prompt (cancellation, agent shutdown, …).
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ApprovalDispatchError {
    /// No pending request matches the supplied id — likely a duplicate
    /// click or a stale GUI handle.
    #[error("no pending approval found for request_id {0}")]
    Unknown(Uuid),
    /// The receiver was dropped before the GUI replied (cancellation,
    /// agent shutdown, …).
    #[error("approval channel for request_id {0} closed before decision arrived")]
    Closed(Uuid),
}

/// What the policy tells the runtime when a call needs human approval.
///
/// `display_parameters` is the redacted preview the GUI should show —
/// the policy is expected to scrub secrets here. `allow_always`
/// indicates whether the "always approve this tool" affordance makes
/// sense (e.g. read-only tools may set it `true`; one-shot destructive
/// tools may force `false`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalRequest {
    /// Human-readable description rendered in the prompt.
    pub description: String,
    /// Sanitised arguments preview for the GUI; never the raw payload
    /// the policy received.
    pub display_parameters: serde_json::Value,
    /// `true` when the GUI may offer an "always approve" affordance.
    pub allow_always: bool,
}

/// Pluggable rule that decides whether a [`ToolCall`] needs human
/// approval.
///
/// Returning `None` means "auto-approve, continue the loop"; returning
/// `Some(req)` causes the headless delegate to emit
/// [`crate::AgentEvent::ApprovalNeeded`] and block until the GUI replies
/// via [`crate::Agent::respond_to_approval`].
#[async_trait]
pub trait ApprovalPolicy: Send + Sync {
    /// Inspect a single tool call and decide if it needs approval.
    async fn evaluate(&self, call: &ToolCall) -> Option<ApprovalRequest>;
}

/// Default policy: every tool call is auto-approved.
///
/// Wired automatically by [`crate::AgentBuilder`] when the caller does
/// not supply a policy, so existing zero-approval behaviour is
/// preserved bit-for-bit.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoApprovalPolicy;

#[async_trait]
impl ApprovalPolicy for NoApprovalPolicy {
    async fn evaluate(&self, _call: &ToolCall) -> Option<ApprovalRequest> {
        None
    }
}

/// Inbox of pending approvals, keyed by request id.
///
/// The runtime owns one [`ApprovalInbox`] per [`crate::Agent`]: the
/// headless delegate registers a one-shot sender before emitting
/// [`crate::AgentEvent::ApprovalNeeded`], and
/// [`crate::Agent::respond_to_approval`] resolves it. Held behind a
/// plain `std::sync::Mutex` because every critical section is a single
/// `HashMap` op — async locking would only add a `Box::pin` allocation
/// per poll.
#[derive(Default, Clone)]
pub struct ApprovalInbox {
    inner: Arc<Mutex<HashMap<Uuid, oneshot::Sender<ApprovalDecision>>>>,
}

impl ApprovalInbox {
    /// Create an empty inbox.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a fresh request and return the receiving end the
    /// delegate should await.
    pub fn register(&self, request_id: Uuid) -> oneshot::Receiver<ApprovalDecision> {
        let (tx, rx) = oneshot::channel();
        // A poisoned mutex here means an earlier panic during inbox
        // mutation; recover the inner state and keep going — a poisoned
        // approval inbox should never take down the whole agent.
        let mut guard = match self.inner.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.insert(request_id, tx);
        rx
    }

    /// Resolve a pending request. Returns the dispatched decision via
    /// the oneshot, or an error if the id is unknown / the receiver
    /// already dropped.
    pub fn dispatch(
        &self,
        request_id: Uuid,
        decision: ApprovalDecision,
    ) -> Result<(), ApprovalDispatchError> {
        let sender = {
            let mut guard = match self.inner.lock() {
                Ok(g) => g,
                Err(poisoned) => poisoned.into_inner(),
            };
            guard
                .remove(&request_id)
                .ok_or(ApprovalDispatchError::Unknown(request_id))?
        };
        sender
            .send(decision)
            .map_err(|_| ApprovalDispatchError::Closed(request_id))
    }

    /// Forget a pending request without dispatching, used when the
    /// receiver side gave up (loop cancellation, executor returning
    /// early, …) so the GUI's late reply maps to
    /// [`ApprovalDispatchError::Unknown`] rather than a silent leak.
    pub fn forget(&self, request_id: Uuid) {
        let mut guard = match self.inner.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.remove(&request_id);
    }
}

/// L5 cross-cutting seam (ADR-160 §3): outcome the runtime receives
/// from an [`Approver`] for a single tool call.
///
/// Crate-internal control flow only; the GUI's wire format remains
/// [`ApprovalDecision`].
#[derive(Debug, PartialEq, Eq)]
pub enum ApprovalOutcome {
    /// No approval policy fired; continue dispatch.
    NotRequired,
    /// User (or default policy) said yes.
    Approved,
    /// User refused; the dispatcher must short-circuit the loop with
    /// an [`crate::AgentError::ApprovalRejected`].
    Rejected { reason: Option<String> },
}

/// L5 cross-cutting seam (ADR-160 §3): asks the host whether a tool
/// call may proceed before the dispatcher runs the executor.
///
/// Implementations are responsible for emitting any user-facing
/// approval prompts (e.g. [`crate::AgentEvent::ApprovalNeeded`]) and
/// for blocking until the host replies. The default in-tree impl is
/// [`PolicyApprover`], which drives an [`ApprovalPolicy`] +
/// [`ApprovalInbox`] pair just like the pre-W6.3 inline pipeline did.
#[async_trait]
pub trait Approver: Send + Sync {
    /// Inspect `call` and return the host's verdict. `event_tx` is the
    /// loop's event channel — implementations may push
    /// [`AgentEvent`]s through it to surface a GUI prompt.
    async fn await_approval(
        &self,
        call: &ToolCall,
        event_tx: Option<&mpsc::Sender<AgentEvent>>,
    ) -> ApprovalOutcome;
}

/// Default in-tree [`Approver`]: drives an [`ApprovalPolicy`] +
/// [`ApprovalInbox`] pair. Behaviour is byte-identical to the
/// pre-W6.3 inline `SequentialDispatcher::await_approval_for`.
pub struct PolicyApprover {
    policy: Arc<dyn ApprovalPolicy>,
    inbox: ApprovalInbox,
}

impl PolicyApprover {
    /// Wire up an approver around an existing policy + inbox pair.
    #[must_use]
    pub fn new(policy: Arc<dyn ApprovalPolicy>, inbox: ApprovalInbox) -> Self {
        Self { policy, inbox }
    }
}

#[async_trait]
impl Approver for PolicyApprover {
    async fn await_approval(
        &self,
        call: &ToolCall,
        event_tx: Option<&mpsc::Sender<AgentEvent>>,
    ) -> ApprovalOutcome {
        let Some(request) = self.policy.evaluate(call).await else {
            return ApprovalOutcome::NotRequired;
        };

        let request_id = Uuid::new_v4();
        let rx = self.inbox.register(request_id);

        if let Some(tx) = event_tx {
            // Best-effort emit; a closed receiver is not fatal.
            let _ = tx
                .send(AgentEvent::ApprovalNeeded {
                    request_id,
                    tool_name: call.name.clone(),
                    tool_arguments: call.arguments.clone(),
                    description: request.description,
                    display_parameters: request.display_parameters,
                    allow_always: request.allow_always,
                })
                .await;
        }

        match rx.await {
            Ok(ApprovalDecision::Approve) | Ok(ApprovalDecision::ApproveAlways) => {
                ApprovalOutcome::Approved
            }
            Ok(ApprovalDecision::Reject { reason }) => ApprovalOutcome::Rejected { reason },
            Err(_) => {
                // Sender dropped without dispatching: forget the entry
                // so a late respond_to_approval call still hits
                // ApprovalDispatchError::Unknown rather than leaking the
                // pending request forever.
                self.inbox.forget(request_id);
                ApprovalOutcome::Rejected {
                    reason: Some(String::from(
                        "approval channel closed before the GUI replied",
                    )),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn inbox_dispatch_resolves_oneshot() {
        let inbox = ApprovalInbox::new();
        let id = Uuid::new_v4();
        let rx = inbox.register(id);
        inbox.dispatch(id, ApprovalDecision::Approve).expect("ok");
        let decision = rx.await.expect("decision arrives");
        assert_eq!(decision, ApprovalDecision::Approve);
    }

    #[tokio::test]
    async fn inbox_dispatch_unknown_id_errors() {
        let inbox = ApprovalInbox::new();
        let err = inbox
            .dispatch(Uuid::new_v4(), ApprovalDecision::Approve)
            .unwrap_err();
        assert!(matches!(err, ApprovalDispatchError::Unknown(_)));
    }

    #[tokio::test]
    async fn inbox_forget_then_dispatch_is_unknown() {
        let inbox = ApprovalInbox::new();
        let id = Uuid::new_v4();
        let _rx = inbox.register(id);
        inbox.forget(id);
        let err = inbox.dispatch(id, ApprovalDecision::Approve).unwrap_err();
        assert!(matches!(err, ApprovalDispatchError::Unknown(_)));
    }

    #[tokio::test]
    async fn inbox_dispatch_after_receiver_dropped_yields_closed() {
        let inbox = ApprovalInbox::new();
        let id = Uuid::new_v4();
        let rx = inbox.register(id);
        drop(rx);
        let err = inbox
            .dispatch(id, ApprovalDecision::Reject { reason: None })
            .unwrap_err();
        assert!(matches!(err, ApprovalDispatchError::Closed(_)));
    }

    #[test]
    fn approval_decision_adjacent_tagged_wire() {
        let approve = serde_json::to_string(&ApprovalDecision::Approve).unwrap();
        assert_eq!(approve, r#"{"kind":"approve"}"#);

        let reject = serde_json::to_string(&ApprovalDecision::Reject {
            reason: Some("nope".into()),
        })
        .unwrap();
        assert_eq!(reject, r#"{"kind":"reject","data":{"reason":"nope"}}"#);

        let always = serde_json::to_string(&ApprovalDecision::ApproveAlways).unwrap();
        assert_eq!(always, r#"{"kind":"approve_always"}"#);

        for original in [
            ApprovalDecision::Approve,
            ApprovalDecision::Reject { reason: None },
            ApprovalDecision::Reject {
                reason: Some("user denied".into()),
            },
            ApprovalDecision::ApproveAlways,
        ] {
            let s = serde_json::to_string(&original).unwrap();
            let back: ApprovalDecision = serde_json::from_str(&s).unwrap();
            assert_eq!(back, original);
        }
    }
}
