//! Headless `JobContext` trait — see ADR-154.
//!
//! `ironclaw::JobContext` is a 27-field god-struct that mixes three
//! concerns: desktop GUI / marketplace bookkeeping (budget, bids, time
//! stamps, state transition history), ironclaw-internal dependencies
//! (`HttpInterceptor`, `tool_output_stash`, feature flags), and the
//! actual "what does a tool need to know about the current job?" subset.
//!
//! This trait extracts **only the third subset** — the fields that
//! `Tool::execute` implementations actually read or write across the
//! workspace (verified by full-tree grep, see ADR-154 §2.1). GUI /
//! marketplace fields stay on the ironclaw side and **do not** appear in
//! this trait, so headless hosts (admin-backend, claw-code,
//! dasclaw_runtime-only consumers) can supply a minimal implementation
//! without pulling in marketplace bookkeeping.
//!
//! ## Status
//!
//! ADR-154 step 1/2: the trait is added here and `ironclaw::JobContext`
//! implements it, but `Tool::execute` still takes `&JobContext` directly
//! — no call site changes yet. ADR-154 step 2/2 will switch the
//! `Tool::execute` signature to `&mut dyn JobContextCore` and mechanically
//! replace every `ctx.field` with `ctx.field()` across the workspace.

use std::collections::HashMap;
use std::sync::Arc;

use uuid::Uuid;

use crate::feature_flags::SharedFeatureFlags;
use crate::job::JobState;
use crate::recording::HttpInterceptor;

/// Minimal job context surface that any `Tool::execute` implementation
/// can rely on.
///
/// See [`module documentation`](self) and ADR-154 for the rationale
/// behind this split.
pub trait JobContextCore: Send + Sync {
    // ── identifiers ──

    /// Unique job ID.
    fn job_id(&self) -> Uuid;
    /// User ID that owns this job (for workspace scoping).
    fn user_id(&self) -> &str;
    /// Channel-specific requester/actor ID, when different from the owner scope.
    fn requester_id(&self) -> Option<&str>;
    /// Conversation ID if linked to a conversation.
    fn conversation_id(&self) -> Option<Uuid>;
    /// Replace the conversation ID (used by sub-agent / session-fork
    /// flows that create a fresh conversation for forked work).
    fn set_conversation_id(&mut self, id: Option<Uuid>);

    // ── state ──

    /// Current job state.
    fn state(&self) -> JobState;

    // ── shared tool I/O ──

    /// Job metadata (read by many tools, written by `job.rs:1886`).
    fn metadata(&self) -> &serde_json::Value;
    /// Replace the job metadata blob wholesale.
    fn set_metadata(&mut self, value: serde_json::Value);
    /// Stash of full tool outputs keyed by tool_call_id, used for
    /// cross-tool reference passing (`$tool_call_id` syntax) and
    /// implicit state (keys prefixed with `__`).
    fn tool_output_stash(&self) -> Arc<tokio::sync::RwLock<HashMap<String, String>>>;
    /// Extra environment variables to inject into spawned child
    /// processes (e.g., fetched credentials passed to shell tools).
    fn extra_env(&self) -> Arc<HashMap<String, String>>;

    // ── environment / recording ──

    /// User's preferred timezone (IANA name, e.g. "America/New_York").
    fn user_timezone(&self) -> &str;
    /// Optional HTTP interceptor for trace recording/replay.
    fn http_interceptor(&self) -> Option<Arc<dyn HttpInterceptor>>;

    // ── capability gates ──

    /// Tool-level feature flags controlling which tools are enabled.
    fn feature_flags(&self) -> SharedFeatureFlags;

    // ── job summary (used by `job.rs` cross-task listing) ──

    /// Job title.
    fn title(&self) -> &str;
    /// Job description.
    fn description(&self) -> &str;
}
