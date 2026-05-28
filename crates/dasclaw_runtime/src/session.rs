//! Multi-turn `Session` facade for [`Agent`] (issue #907 Action 2).
//!
//! ## Why this exists
//!
//! A bare [`Agent::run`] is one-shot: every call seeds a fresh
//! `ReasoningContext`, so the GUI has no place to keep "the prior turn"
//! short of re-feeding the full history string into the next prompt. The
//! issue #907 background calls this out as a GUI blocker.
//!
//! `Session` is the smallest thing that fixes it:
//!
//! - holds an `Arc<Agent>` plus one persistent [`ReasoningContext`]
//! - on construction, seeds the context once with the agent's system
//!   prompt / model override / advertised tools (via
//!   [`Agent::seed_context`])
//! - each [`Session::run`] appends a new user turn to the same context
//!   and re-enters the loop, so the responder sees the full conversation
//!
//! Cancellation is inherited from the agent: if the agent was built with
//! [`crate::AgentBuilder::cancellation_token`], cancelling the handle
//! halts the in-flight `Session::run` with [`AgentError::Stopped`] at
//! the next loop signal check.
//!
//! ## Non-goals
//!
//! - **No serde / persistence / fork / compaction.** Those land in a
//!   follow-up issue that ports the rich `claw-code::Session` features
//!   into a dedicated `dasclaw_session` crate (ADR-153 §4.3 B+).
//! - **No streaming.** See issue B2.
//!
//! ## Example
//!
//! ```ignore
//! use std::sync::Arc;
//! use dasclaw_runtime::{Agent, Session};
//!
//! let agent = Arc::new(
//!     Agent::builder()
//!         .responder(my_responder)
//!         .system_prompt("be concise")
//!         .build()?,
//! );
//! let mut session = Session::new(agent);
//! let reply1 = session.run("what's 2 + 2?").await?;
//! let reply2 = session.run("and times 10?").await?; // sees prior turn
//! ```

use std::sync::Arc;

use dasclaw_core::messages::ChatMessage;
use dasclaw_core::reasoning_ctx::ReasoningContext;

use crate::agent::{Agent, AgentError};

/// Stateful, multi-turn conversation handle around an [`Agent`].
///
/// Keeps a single [`ReasoningContext`] alive across calls so each
/// [`Session::run`] carries the previous user + assistant + tool_result
/// turns into the next loop iteration.
pub struct Session {
    agent: Arc<Agent>,
    ctx: ReasoningContext,
}

impl Session {
    /// Build a new session bound to `agent`. The session's
    /// [`ReasoningContext`] is seeded once with the agent's system
    /// prompt, model override and advertised tools.
    #[must_use]
    pub fn new(agent: Arc<Agent>) -> Self {
        let mut ctx = ReasoningContext::new();
        agent.seed_context(&mut ctx);
        Self { agent, ctx }
    }

    /// Send a new user prompt and return the final text response.
    ///
    /// The prompt is appended to the session's accumulated history, then
    /// the agentic loop runs as in [`Agent::run`]. All assistant turns,
    /// tool calls and tool results produced by the loop remain in the
    /// session's context for subsequent calls.
    pub async fn run(&mut self, prompt: &str) -> Result<String, AgentError> {
        self.agent.run_in_context(&mut self.ctx, prompt).await
    }

    /// Read-only view of the accumulated conversation history. Useful
    /// for a GUI that needs to render the dialog or for tests that
    /// assert on what the responder saw.
    #[must_use]
    pub fn messages(&self) -> &[ChatMessage] {
        &self.ctx.messages
    }
}
