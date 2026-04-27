//! Response payloads returned by LLM providers, shared between the agentic
//! loop and its delegates.
//!
//! These types were lifted out of `ironclaw::llm::reasoning` as part of
//! Phase 3 Step D-4 so the agentic loop can live in `x_claw_agent` without
//! pulling in the `Reasoning` engine (which depends on `LlmProvider` /
//! `rust_decimal` / `LlmError`).

use crate::messages::{FinishReason, ToolCall};

/// Token usage from a single LLM call.
#[derive(Debug, Clone, Copy, Default)]
pub struct TokenUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    /// Tokens served from the provider's server-side prompt cache (Anthropic).
    pub cache_read_input_tokens: u32,
    /// Tokens written to the provider's prompt cache (Anthropic).
    pub cache_creation_input_tokens: u32,
}

impl TokenUsage {
    pub fn total(&self) -> u32 {
        self.input_tokens + self.output_tokens
    }
}

/// Structured anomaly classification for LLM responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseAnomaly {
    /// Tool mode was requested, but the provider returned no usable tool calls
    /// and no recoverable text content.
    EmptyToolCompletion,
    /// Text mode returned no usable content after cleaning/truncation.
    EmptyTextResponse,
}

/// Metadata attached to `RespondOutput` so callers can react to malformed
/// provider behavior without inferring it from fallback strings.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ResponseMetadata {
    pub anomaly: Option<ResponseAnomaly>,
}

/// Result of a response with potential tool calls.
///
/// Used by the agent loop to handle tool execution before returning a final
/// response.
#[derive(Debug, Clone)]
pub enum RespondResult {
    /// A text response (no tools needed).
    Text(String),
    /// The model wants to call tools. Caller should execute them and call back.
    /// Includes the optional content from the assistant message (some models
    /// include explanatory text alongside tool calls).
    ToolCalls {
        tool_calls: Vec<ToolCall>,
        content: Option<String>,
    },
}

/// A `RespondResult` bundled with the token usage from the LLM call that
/// produced it.
#[derive(Debug, Clone)]
pub struct RespondOutput {
    pub result: RespondResult,
    pub usage: TokenUsage,
    pub finish_reason: FinishReason,
    pub metadata: ResponseMetadata,
}
