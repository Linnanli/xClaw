//! Multi-turn `Session` facade for [`Agent`] + persistence boundary.
//!
//! This crate is the home of the conversation-level vocabulary on top
//! of `dasclaw_runtime`'s one-shot [`Agent::run`]:
//!
//! - [`Session`] — stateful multi-turn handle around `Arc<Agent>`.
//!   Carries a [`SessionSnapshot`] of metadata + messages across
//!   `run()` calls so the responder always sees the prior turns.
//! - [`SessionSnapshot`] — serializable view of everything a session
//!   needs to survive a process restart. The wire format is locked by
//!   an insta snapshot test.
//! - [`SessionStore`] — async trait that store backends implement.
//!   This crate ships [`InMemorySessionStore`]; the jsonl-on-disk
//!   store (claw-code parity, with rotation + cleanup) lands in #914
//!   PR-C.
//! - [`SessionError`] — single concrete error type for all
//!   persistence and snapshot operations.
//!
//! Cancellation is inherited from the agent: if the agent was built
//! with [`dasclaw_runtime::AgentBuilder::cancellation_token`],
//! cancelling the handle halts the in-flight [`Session::run`] with
//! [`AgentError::Stopped`] at the next loop signal check.
//!
//! ## Non-goals (phase 2)
//!
//! - **No fork / compaction** — see issue #914 PR-D.
//! - **No prompt_history** — see issue #914 PR-D.
//! - **No streaming** — see issue B2.
//! - **No on-disk store** — see issue #914 PR-C.
//!
//! ## Example
//!
//! ```ignore
//! use std::sync::Arc;
//! use dasclaw_runtime::Agent;
//! use dasclaw_session::{InMemorySessionStore, Session, SessionStore};
//!
//! let agent = Arc::new(
//!     Agent::builder()
//!         .responder(my_responder)
//!         .system_prompt("be concise")
//!         .build()?,
//! );
//!
//! // Drive a conversation.
//! let mut session = Session::new(agent.clone()).with_model("claude-opus");
//! let _reply1 = session.run("what's 2 + 2?").await?;
//! let _reply2 = session.run("and times 10?").await?;
//!
//! // Persist + resume.
//! let store: Arc<dyn SessionStore> = Arc::new(InMemorySessionStore::new());
//! store.save(session.snapshot()).await?;
//! let snap = store.load(session.session_id()).await?.expect("stored");
//! let resumed = Session::from_snapshot(agent, snap);
//! assert_eq!(resumed.messages().len(), session.messages().len());
//! ```

pub mod error;
pub mod id;
pub mod jsonl;
pub mod snapshot;
pub mod store;

pub use error::SessionError;
pub use id::{SESSION_VERSION, generate_session_id};
pub use jsonl::{JsonlSessionStore, MAX_ROTATED_FILES, ROTATE_AFTER_BYTES};
pub use snapshot::{SessionCompaction, SessionMetadata, SessionSnapshot};
pub use store::{InMemorySessionStore, SessionStore};

use std::path::PathBuf;
use std::sync::Arc;

use dasclaw_core::messages::ChatMessage;
use dasclaw_core::reasoning_ctx::ReasoningContext;
use dasclaw_runtime::{Agent, AgentError};

/// Stateful, multi-turn conversation handle around an [`Agent`].
///
/// Owns a [`SessionSnapshot`] (the persistable state) plus an
/// `Arc<Agent>` (the live executor). Each [`Session::run`] builds a
/// fresh [`ReasoningContext`] for the agentic loop, replays the
/// session's accumulated messages into it, appends the new prompt,
/// runs to completion, and copies the resulting messages back into
/// the snapshot.
///
/// The snapshot is the only thing a [`SessionStore`] needs to
/// persist; the agent is reconstructed at resume time by the host.
pub struct Session {
    agent: Arc<Agent>,
    state: SessionSnapshot,
}

impl Session {
    /// Build a fresh session bound to `agent`, with a freshly
    /// generated [`SessionSnapshot::fresh`] (new id, current time,
    /// empty history).
    #[must_use]
    pub fn new(agent: Arc<Agent>) -> Self {
        Self {
            agent,
            state: SessionSnapshot::fresh(),
        }
    }

    /// Resume a session from a previously stored snapshot. The
    /// caller is responsible for rebuilding an `Arc<Agent>` whose
    /// configuration matches the original session's expectations
    /// (model, tool catalogue, etc.).
    #[must_use]
    pub fn from_snapshot(agent: Arc<Agent>, state: SessionSnapshot) -> Self {
        Self { agent, state }
    }

    /// Record the model name on the snapshot. Builder-style; useful
    /// at session construction:
    /// `Session::new(agent).with_model("claude-opus")`.
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.state.model = Some(model.into());
        self
    }

    /// Record the workspace root on the snapshot. Builder-style.
    #[must_use]
    pub fn with_workspace_root(mut self, path: impl Into<PathBuf>) -> Self {
        self.state.workspace_root = Some(path.into());
        self
    }

    /// Read-only view of the persistable state. Hand this to a
    /// [`SessionStore::save`] call.
    #[must_use]
    pub fn snapshot(&self) -> &SessionSnapshot {
        &self.state
    }

    /// This session's stable identifier.
    #[must_use]
    pub fn session_id(&self) -> &str {
        &self.state.session_id
    }

    /// Read-only view of the accumulated conversation history.
    #[must_use]
    pub fn messages(&self) -> &[ChatMessage] {
        &self.state.messages
    }

    /// Send a new user prompt and return the final text response.
    ///
    /// On every call this method:
    ///
    /// 1. builds a fresh [`ReasoningContext`] via
    ///    [`Agent::seed_context`] so the static configuration
    ///    (system prompt, available tools, model override) is always
    ///    re-applied,
    /// 2. replays the snapshot's prior messages into the context,
    /// 3. delegates to [`Agent::run_in_context`], which appends the
    ///    user prompt and drives the agentic loop to completion,
    /// 4. copies the resulting messages back into the snapshot —
    ///    even on failure, so a partial conversation is preserved,
    /// 5. on success, advances [`SessionSnapshot::updated_at_ms`].
    pub async fn run(&mut self, prompt: &str) -> Result<String, AgentError> {
        let mut ctx = ReasoningContext::new();
        self.agent.seed_context(&mut ctx);
        ctx.messages = std::mem::take(&mut self.state.messages);

        let result = self.agent.run_in_context(&mut ctx, prompt).await;

        self.state.messages = std::mem::take(&mut ctx.messages);
        if result.is_ok() {
            self.state.touch();
        }
        result
    }

    /// Compact the conversation by dropping the oldest `remove_count`
    /// messages and inserting a single summary system message at the
    /// front. Delegates to [`SessionSnapshot::compact_oldest`];
    /// touches `updated_at_ms` when a non-trivial pass actually runs.
    pub fn compact_oldest<F>(&mut self, remove_count: usize, summarizer: F)
    where
        F: FnOnce(&[ChatMessage]) -> String,
    {
        let prior_pass_count = self.state.compaction.as_ref().map(|c| c.count);
        self.state.compact_oldest(remove_count, summarizer);
        let new_pass_count = self.state.compaction.as_ref().map(|c| c.count);
        if prior_pass_count != new_pass_count {
            self.state.touch();
        }
    }
}
