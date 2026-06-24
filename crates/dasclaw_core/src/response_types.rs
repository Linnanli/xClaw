//! Response payloads returned by LLM providers, shared between the agentic
//! loop and its delegates.
//!
//! These types were lifted out of `ironclaw::llm::reasoning` as part of
//! Phase 3 Step D-4 so the agentic loop can live in `dasclaw_core` without
//! pulling in the `Reasoning` engine (which depends on `LlmProvider` /
//! `rust_decimal` / `LlmError`).

use serde::{Deserialize, Serialize};

pub use crate::messages::TokenUsage;
use crate::messages::{FinishReason, ToolCall};

/// Structured anomaly classification for LLM responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum ResponseAnomaly {
    /// Tool mode was requested, but the provider returned no usable tool calls
    /// and no recoverable text content.
    EmptyToolCompletion,
    /// Text mode returned no usable content after cleaning/truncation.
    EmptyTextResponse,
}

/// Provider-side verification signals attached to a response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResponseModelVerification {
    TrustedAccessForCyber,
}

/// Metadata attached to `RespondOutput` so callers can react to malformed
/// provider behavior without inferring it from fallback strings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseMetadata {
    #[serde(default)]
    pub anomaly: Option<ResponseAnomaly>,
    #[serde(default)]
    pub actual_model: Option<String>,
    #[serde(default)]
    pub model_verifications: Vec<ResponseModelVerification>,
}

/// Result of a response with potential tool calls.
///
/// Used by the agent loop to handle tool execution before returning a final
/// response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RespondOutput {
    pub result: RespondResult,
    pub usage: TokenUsage,
    pub finish_reason: FinishReason,
    pub metadata: ResponseMetadata,
}
