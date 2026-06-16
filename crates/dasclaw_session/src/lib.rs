//! Multi-turn `Session` facade for [`Agent`] + persistence boundary.
//!
//! This crate is the home of the conversation-level vocabulary on top
//! of `dasclaw_runtime`'s one-shot [`Agent::invoke`]:
//!
//! - [`Session`] — stateful multi-turn handle around `Arc<Agent>`.
//!   Carries a [`SessionSnapshot`] of metadata + messages across
//!   `invoke()` calls so the responder always sees the prior turns.
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
//! cancelling the handle halts the in-flight [`Session::invoke`] with
//! [`AgentError::Stopped`] at the next loop signal check.
//!
//! ## Non-goals (phase 2)
//!
//! - **No fork / compaction** — see issue #914 PR-D.
//! - **No prompt_history** — see issue #914 PR-D.
//! - **No on-disk store** — see issue #914 PR-C.
//!
//! ## Extending session state without modifying `SessionSnapshot`
//!
//! Per ADR-160 / doc 56 §2.2, [`SessionSnapshot`] is a concrete struct,
//! **not** an extension-slotted blob (no `extensions: serde_json::Value`,
//! no `metadata: Map<String, Value>`). Third-party agents that need to
//! persist extra state alongside the framework's session should use the
//! **`WrappedSession`** pattern:
//!
//! ```ignore
//! struct MyWrappedSession {
//!     inner: SessionSnapshot,       // framework-owned, persisted via SessionStore
//!     my_custom_state: MyState,     // wrapper-owned, persisted separately
//! }
//! ```
//!
//! The framework persists `inner` via [`SessionStore`]; the wrapper
//! persists `my_custom_state` independently (its own JSONL file, sidecar
//! table, etc.). When a field is meaningful to *all* agents, propose
//! adding it to `SessionSnapshot` directly and bump [`SESSION_VERSION`]
//! via the migration scaffolding in [`migration`].
//!
//! See `desktop-client/ironclaw` for a real wrapper that layers extension
//! state on top of the framework snapshot.
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
//! let _reply1 = session.invoke("what's 2 + 2?", AgentRunOptions::invoke()).await?;
//! let _reply2 = session.invoke("and times 10?", AgentRunOptions::invoke()).await?;
//!
//! // Persist + resume.
//! let store: Arc<dyn SessionStore> = Arc::new(InMemorySessionStore::new());
//! store.save(session.snapshot()).await?;
//! let snap = store.load(session.session_id()).await?.expect("stored");
//! let resumed = Session::from_snapshot(agent, snap);
//! assert_eq!(resumed.messages().len(), session.messages().len());
//! ```

pub mod claw_compat;
pub mod error;
pub mod id;
pub mod jsonl;
pub mod migration;
pub mod snapshot;
pub mod store;

pub use claw_compat::{
    ClawContentBlock, ClawMessage, ClawRole, chat_message_to_claw, claw_to_chat_message,
};
pub use error::SessionError;
pub use id::{SESSION_VERSION, generate_session_id};
pub use jsonl::{JsonlSessionStore, MAX_ROTATED_FILES, ROTATE_AFTER_BYTES};
pub use migration::{MigrationError, migrate_to_latest, migrate_v1_to_v2};
pub use snapshot::{
    SessionCompaction, SessionFork, SessionMetadata, SessionPromptEntry, SessionSnapshot,
};
pub use store::{InMemorySessionStore, SessionStore};

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use dasclaw_core::messages::ChatMessage;
use dasclaw_core::reasoning_ctx::ReasoningContext;
use dasclaw_runtime::{Agent, AgentError, AgentEvent, AgentRunOptions, AgentRunOutput};
use tokio::sync::{mpsc, oneshot};

/// Event stream returned by [`Session::stream`].
pub struct SessionRunStream<'a> {
    session: &'a mut Session,
    event_rx: mpsc::Receiver<AgentEvent>,
    result_rx: oneshot::Receiver<(ReasoningContext, Result<AgentRunOutput, AgentError>)>,
    done: bool,
}

impl futures_core::Stream for SessionRunStream<'_> {
    type Item = Result<AgentEvent, AgentError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        if this.done {
            return Poll::Ready(None);
        }

        match Pin::new(&mut this.event_rx).poll_recv(cx) {
            Poll::Ready(Some(event)) => return Poll::Ready(Some(Ok(event))),
            Poll::Ready(None) => {}
            Poll::Pending => {
                if let Poll::Ready(item) = poll_session_result(this, cx) {
                    return Poll::Ready(item);
                }
                return Poll::Pending;
            }
        }

        poll_session_result(this, cx)
    }
}

fn poll_session_result(
    stream: &mut SessionRunStream<'_>,
    cx: &mut Context<'_>,
) -> Poll<Option<Result<AgentEvent, AgentError>>> {
    match Pin::new(&mut stream.result_rx).poll(cx) {
        Poll::Ready(Ok((mut ctx, result))) => {
            stream.session.state.messages = std::mem::take(&mut ctx.messages);
            if result.is_ok() {
                stream.session.state.touch();
            }
            stream.done = true;
            Poll::Ready(Some(result.map(AgentEvent::Completed)))
        }
        Poll::Ready(Err(_)) => {
            stream.done = true;
            Poll::Ready(Some(Err(AgentError::LoopFailure(
                "session stream producer task ended before completion".to_string(),
            ))))
        }
        Poll::Pending => Poll::Pending,
    }
}

/// Stateful, multi-turn conversation handle around an [`Agent`].
///
/// Owns a [`SessionSnapshot`] (the persistable state) plus an
/// `Arc<Agent>` (the live executor). Each [`Session::invoke`] builds a
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

    /// Send a new user prompt and return the final output.
    ///
    /// On every call this method:
    ///
    /// 1. builds a fresh [`ReasoningContext`] via
    ///    [`Agent::seed_context`] so the static configuration
    ///    (system prompt, available tools, model override) is always
    ///    re-applied,
    /// 2. replays the snapshot's prior messages into the context,
    /// 3. delegates to [`Agent::invoke_in_context`], which appends the
    ///    user prompt and drives the agentic loop to completion,
    /// 4. copies the resulting messages back into the snapshot —
    ///    even on failure, so a partial conversation is preserved,
    /// 5. on success, advances [`SessionSnapshot::updated_at_ms`].
    pub async fn invoke(
        &mut self,
        prompt: &str,
        options: AgentRunOptions,
    ) -> Result<AgentRunOutput, AgentError> {
        let mut ctx = ReasoningContext::new();
        self.agent.seed_context(&mut ctx);
        ctx.messages = std::mem::take(&mut self.state.messages);

        let result = self
            .agent
            .invoke_in_context(&mut ctx, prompt, options)
            .await;

        self.state.messages = std::mem::take(&mut ctx.messages);
        if result.is_ok() {
            self.state.touch();
        }
        result
    }

    /// Stream a new user prompt through the session.
    ///
    /// The returned stream yields agent events and ends a successful turn
    /// with [`AgentEvent::Completed`]. The session history is written back
    /// when the producer task reaches a terminal result.
    pub fn stream(&mut self, prompt: &str, options: AgentRunOptions) -> SessionRunStream<'_> {
        let mut ctx = ReasoningContext::new();
        self.agent.seed_context(&mut ctx);
        ctx.messages = self.state.messages.clone();

        let agent = Arc::clone(&self.agent);
        let prompt = prompt.to_string();
        let (event_tx, event_rx) = mpsc::channel::<AgentEvent>(64);
        let (result_tx, result_rx) =
            oneshot::channel::<(ReasoningContext, Result<AgentRunOutput, AgentError>)>();

        tokio::spawn(async move {
            let result = agent
                .stream_in_context(&mut ctx, &prompt, options, event_tx)
                .await;
            let _ = result_tx.send((ctx, result));
        });

        SessionRunStream {
            session: self,
            event_rx,
            result_rx,
            done: false,
        }
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

    /// Record `text` as a user prompt in `prompt_history`.
    /// Delegates to [`SessionSnapshot::record_prompt`].
    pub fn record_prompt(&mut self, text: impl Into<String>) {
        self.state.record_prompt(text);
    }

    /// Create a forked `Session` that shares this session's [`Agent`]
    /// but carries an independent [`SessionSnapshot`] descended from
    /// the current one (see [`SessionSnapshot::fork`]).
    #[must_use]
    pub fn fork(&self, branch_name: Option<String>) -> Self {
        Self {
            agent: self.agent.clone(),
            state: self.state.fork(branch_name),
        }
    }
}
