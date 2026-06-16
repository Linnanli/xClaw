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
//! - Streaming / interactive REPL. The first slice is request-response.
//!
//! ## Real LLM provider wiring
//!
//! See [`provider`] for the [`ProviderArgs`](provider::ProviderArgs) →
//! [`LlmProviderResponder`](dasclaw_runtime::LlmProviderResponder)
//! factory used by the binary's non-echo mode (ADR-153 §4.4 step 5).
//!
//! ## Tool dispatch
//!
//! See [`tools`] for the in-memory [`StaticToolExecutor`](tools::StaticToolExecutor)
//! used by `dasclaw-cli run --enable-tools` (ADR-153 §4.4 step 6).
//! Drive it through [`run_with_tools`] when the agent should be allowed
//! to call tools.
//!
//! ## Hook wiring (W6.1)
//!
//! See [`run_with_tools_and_hooks`] for the entry that lets callers
//! inject a [`dasclaw_core::hooks::HookBundle`] (EgressGate, approval,
//! sandbox, audit). This is the single point at which the 9-layer
//! defence stack from `13-security-capability-inventory.md` can be
//! opted-in from a headless caller.
//!
//! ## Default tool-output sanitization (ADR-153 §1.1 B6 / e22 / #882)
//!
//! See [`run_with_tools_and_safety_sanitizer`] for the entry the
//! shipped binary uses by default. It wires
//! [`sanitizer::SafetyStashSanitizer`] through
//! [`dasclaw_runtime::ToolOutputSanitizer`] so `tool_result` payloads
//! are redacted **before** the next LLM call observes them. The plain
//! [`run_with_tools`] entry stays as the no-sanitizer escape hatch so
//! the e22 negative-control test can still pin the runtime's
//! forward-verbatim contract.

use std::sync::Mutex;

use async_trait::async_trait;
use dasclaw_core::hooks::HookBundle;
use dasclaw_core::messages::{FinishReason, Role, ToolDefinition};
use dasclaw_core::reasoning_ctx::ReasoningContext;
use dasclaw_core::response_types::{RespondOutput, RespondResult, ResponseMetadata, TokenUsage};
use dasclaw_core::traits::HostError;
use dasclaw_runtime::{Agent, AgentError, AgentResponder, AgentRunOptions, ToolExecutor};

pub mod mcp;
pub mod provider;
pub mod sandbox_exec;
pub mod sanitizer;
pub mod tools;
#[cfg(feature = "wasm-tools")]
pub mod wasm;

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
    let reply = agent.invoke(user_prompt, AgentRunOptions::invoke()).await?;
    Ok(reply.text)
}

/// Variant of [`run`] that wires a [`ToolExecutor`] and advertises its
/// tool definitions to the model.
///
/// `tool_definitions` is normally [`tools::StaticToolExecutor::definitions`].
/// Keeping [`run`] and `run_with_tools` as separate entries avoids a
/// patch-style optional parameter on the original signature and keeps
/// the no-tools call-path free of executor plumbing.
pub async fn run_with_tools<R, E>(
    responder: R,
    tool_executor: E,
    tool_definitions: Vec<ToolDefinition>,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<String, CliError>
where
    R: AgentResponder + 'static,
    E: ToolExecutor + 'static,
{
    let agent: Agent = Agent::builder()
        .responder(responder)
        .tool_executor(tool_executor)
        .tools(tool_definitions)
        .system_prompt(system_prompt)
        .build()
        .map_err(|e| CliError::Build(e.to_string()))?;
    let reply = agent.invoke(user_prompt, AgentRunOptions::invoke()).await?;
    Ok(reply.text)
}

/// Variant of [`run_with_tools`] that also wires a [`HookBundle`].
///
/// The hook bundle is the only path through which a headless caller can
/// opt into the layered defences enumerated in
/// `docs/plans/architecture-refactor/13-security-capability-inventory.md`
/// (EgressGate, approval, sandbox bridge, audit, …). Without this entry
/// the only available bundle is [`HookBundle::noop`], which makes the
/// library "unsafe by default and impossible to opt-in to safety" — see
/// ADR-153-cli-test-matrix §1.3 G1 for the full rationale.
///
/// Like [`run`] and [`run_with_tools`], this is intentionally a separate
/// entry rather than an optional parameter on `run_with_tools`. The
/// existing entries stay byte-for-byte stable so the e1–e6 integration
/// tests continue to exercise the noop-hook path verbatim.
pub async fn run_with_tools_and_hooks<R, E>(
    responder: R,
    tool_executor: E,
    tool_definitions: Vec<ToolDefinition>,
    hooks: HookBundle,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<String, CliError>
where
    R: AgentResponder + 'static,
    E: ToolExecutor + 'static,
{
    let agent: Agent = Agent::builder()
        .responder(responder)
        .tool_executor(tool_executor)
        .tools(tool_definitions)
        .hooks(hooks)
        .system_prompt(system_prompt)
        .build()
        .map_err(|e| CliError::Build(e.to_string()))?;
    let reply = agent.invoke(user_prompt, AgentRunOptions::invoke()).await?;
    Ok(reply.text)
}

/// Variant of [`run_with_tools`] that wires the default
/// [`sanitizer::SafetyStashSanitizer`] before handing tool output back
/// to the LLM (ADR-153 §1.1 B6 / e22 / issue #882).
///
/// This is the entry the binary now uses for tool-enabled runs, so the
/// shipped `dasclaw-cli` never feeds raw tool output to a model. The
/// existing [`run_with_tools`] entry stays byte-for-byte stable so
/// downstream callers that want **no** sanitizer (the verbatim
/// `tool_result` contract checked by the e22 negative-control test) can
/// still opt in by routing through the original signature.
///
/// Like [`run_with_tools_and_hooks`], this is a separate entry rather
/// than an optional parameter. The CLI's `run_*` family has standardised
/// on additive entries so each new capability adds one well-typed path
/// instead of growing a combinatorial signature.
pub async fn run_with_tools_and_safety_sanitizer<R, E>(
    responder: R,
    tool_executor: E,
    tool_definitions: Vec<ToolDefinition>,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<String, CliError>
where
    R: AgentResponder + 'static,
    E: ToolExecutor + 'static,
{
    let agent: Agent = Agent::builder()
        .responder(responder)
        .tool_executor(tool_executor)
        .tools(tool_definitions)
        .tool_output_sanitizer_arc(sanitizer::default_safety_sanitizer())
        .system_prompt(system_prompt)
        .build()
        .map_err(|e| CliError::Build(e.to_string()))?;
    let reply = agent.invoke(user_prompt, AgentRunOptions::invoke()).await?;
    Ok(reply.text)
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
        // Mutex poisoning here only happens if a previous holder panicked.
        // The counter has no invariants to protect, so recovering the inner
        // value is safe and lets us stay panic-free in production code.
        *self
            .call_count
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
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
        // See call_count(): a poisoned mutex around a plain counter has no
        // invariants to protect, so recover instead of panicking.
        *self
            .call_count
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) += 1;
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
