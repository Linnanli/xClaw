//! Per-invocation reasoning context passed through the agentic loop.
//!
//! Ported from `ironclaw::llm::reasoning::ReasoningContext` as part of
//! Phase 3 Step D-4. The `Reasoning` engine itself stays in ironclaw
//! because it depends on `LlmProvider` / `Decimal` / `LlmError`. Only the
//! **data** that flows through the loop moves here.

use std::collections::HashMap;

use crate::messages::{ChatMessage, ToolDefinition};

/// Context for reasoning operations.
///
/// This is a mutable per-turn bundle that the agentic loop and its delegate
/// read and mutate together: it holds the running message list, the current
/// tool definitions, cached metadata, and a few sticky flags that control
/// how the next LLM call is issued.
#[derive(Debug, Clone, Default)]
pub struct ReasoningContext {
    /// Conversation history.
    pub messages: Vec<ChatMessage>,
    /// Available tools.
    pub available_tools: Vec<ToolDefinition>,
    /// Job description if working on a job.
    pub job_description: Option<String>,
    /// Current state description.
    pub current_state: Option<String>,
    /// Opaque metadata forwarded to the LLM provider (e.g. thread_id for
    /// chaining).
    pub metadata: HashMap<String, String>,
    /// When true, force a text-only response (ignore available tools).
    /// Used by the agentic loop to guarantee termination near the iteration
    /// limit. Sticky: once set, never cleared within a loop invocation.
    /// Callers must create a fresh `ReasoningContext` per loop invocation.
    pub force_text: bool,
    /// Pre-built system prompt. When set, the engine uses this directly
    /// instead of building one from its own state. Allows callers to build
    /// the prompt once and reuse it across iterations.
    pub system_prompt: Option<String>,
    /// Per-user model override. When set, completion requests use this model
    /// instead of the provider's default. Only effective with providers that
    /// support per-request model overrides.
    pub model_override: Option<String>,
}

impl ReasoningContext {
    /// Create a new reasoning context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a message to the context.
    pub fn with_message(mut self, message: ChatMessage) -> Self {
        self.messages.push(message);
        self
    }

    /// Set messages directly (for session-based context).
    pub fn with_messages(mut self, messages: Vec<ChatMessage>) -> Self {
        self.messages = messages;
        self
    }

    /// Set available tools.
    pub fn with_tools(mut self, tools: Vec<ToolDefinition>) -> Self {
        self.available_tools = tools;
        self
    }

    /// Set a pre-built system prompt.
    pub fn with_system_prompt(mut self, prompt: String) -> Self {
        self.system_prompt = Some(prompt);
        self
    }

    /// Set job description.
    pub fn with_job(mut self, description: impl Into<String>) -> Self {
        self.job_description = Some(description.into());
        self
    }

    /// Set metadata (forwarded to the LLM provider).
    pub fn with_metadata(mut self, metadata: HashMap<String, String>) -> Self {
        self.metadata = metadata;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_defaults_are_empty() {
        let ctx = ReasoningContext::new();
        assert!(ctx.messages.is_empty());
        assert!(ctx.available_tools.is_empty());
        assert!(!ctx.force_text);
        assert!(ctx.system_prompt.is_none());
        assert!(ctx.model_override.is_none());
    }

    #[test]
    fn builders_chain() {
        let mut md = HashMap::new();
        md.insert("thread_id".into(), "abc".into());
        let ctx = ReasoningContext::new()
            .with_message(ChatMessage::user("hi"))
            .with_job("repro bug")
            .with_metadata(md)
            .with_system_prompt("sys".into());
        assert_eq!(ctx.messages.len(), 1);
        assert_eq!(ctx.job_description.as_deref(), Some("repro bug"));
        assert_eq!(ctx.metadata.get("thread_id").map(String::as_str), Some("abc"));
        assert_eq!(ctx.system_prompt.as_deref(), Some("sys"));
    }
}
