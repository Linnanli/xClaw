//! `dasclaw_cli` — headless agent CLI (ADR-153 §4.4 step 4).
//!
//! This crate is the proof-point that [`dasclaw_runtime::Agent`] runs
//! without any desktop dependency: no Tauri, no database, no channels, no
//! HTTP server. The binary in `src/main.rs` takes a prompt on argv or
//! stdin and prints the agent's reply on stdout.
//!
//! ## Scope of this skeleton (step 4, slice 1)
//!
//! - [`run`] — pure async function. Takes any
//!   [`dasclaw_runtime::AgentResponder`] plus system / user prompts and
//!   returns the agent's text reply. Does no I/O.
//! - [`EchoResponder`] — deterministic built-in
//!   [`AgentResponder`] that replies with `echo: <last user content>`. It
//!   exists solely to let the binary and integration tests demonstrate
//!   end-to-end wire-up without a real LLM key.
//!
//! ## Out of scope (deliberately not in this slice)
//!
//! - Real LLM provider wiring (would expose
//!   `dasclaw_llm_provider` config surface; lands in a follow-up PR once
//!   provider selection mechanism is settled).
//! - Tool execution wiring ([`dasclaw_runtime::ToolExecutor`]). Same
//!   reason.
//! - Streaming / interactive REPL. The first slice is request-response.

use std::sync::Mutex;

use async_trait::async_trait;
use dasclaw_core::messages::{FinishReason, Role};
use dasclaw_core::reasoning_ctx::ReasoningContext;
use dasclaw_core::response_types::{RespondOutput, RespondResult, ResponseMetadata, TokenUsage};
use dasclaw_core::traits::HostError;
use dasclaw_runtime::{Agent, AgentError, AgentResponder};

/// Errors surfaced by the CLI library layer.
#[derive(Debug, thiserror::Error)]
pub enum CliError {
    /// Agent loop returned an error.
    #[error("agent error: {0}")]
    Agent(#[from] AgentError),

    /// Builder rejected the configuration (e.g. missing responder).
    #[error("agent builder error: {0}")]
    Build(String),
}

/// Build an [`Agent`] from `responder` + `system_prompt` and run it once
/// against `user_prompt`, returning the text reply.
///
/// Single library entry point used by both the binary in `src/main.rs`
/// and integration tests under `tests/`. Keeping the I/O (stdin / stdout
/// / argv) out of this function is what lets the integration tests
/// assert behaviour without spawning subprocesses.
pub async fn run<R>(
    responder: R,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<String, CliError>
where
    R: AgentResponder + 'static,
{
    let agent: Agent = Agent::builder()
        .responder(responder)
        .system_prompt(system_prompt)
        .build()
        .map_err(|e| CliError::Build(e.to_string()))?;
    let reply = agent.run(user_prompt).await?;
    Ok(reply)
}

/// Deterministic [`AgentResponder`] that replies `echo: <last user
/// content>`.
///
/// Intended for smoke-testing the headless wiring without an LLM key.
/// Production callers should plug in a real [`AgentResponder`] (e.g.
/// [`dasclaw_runtime::LlmProviderResponder`]).
pub struct EchoResponder {
    call_count: Mutex<usize>,
}

impl EchoResponder {
    /// Create a new echo responder.
    pub fn new() -> Self {
        Self {
            call_count: Mutex::new(0),
        }
    }

    /// Number of times [`AgentResponder::respond`] has been called.
    pub fn call_count(&self) -> usize {
        *self.call_count.lock().expect("call_count mutex poisoned")
    }
}

impl Default for EchoResponder {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AgentResponder for EchoResponder {
    async fn respond(&self, ctx: &mut ReasoningContext) -> Result<RespondOutput, HostError> {
        *self.call_count.lock().expect("call_count mutex poisoned") += 1;
        let last_user_text = ctx
            .messages
            .iter()
            .rev()
            .find(|m| matches!(m.role, Role::User))
            .map(|m| m.content.clone())
            .unwrap_or_default();
        Ok(RespondOutput {
            result: RespondResult::Text(format!("echo: {last_user_text}")),
            usage: TokenUsage::default(),
            finish_reason: FinishReason::Stop,
            metadata: ResponseMetadata::default(),
        })
    }
}
