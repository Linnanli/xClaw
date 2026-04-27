//! A multi-turn LLM stub that returns pre-programmed responses in sequence.
//!
//! Unlike [`StubLlm`](super::StubLlm) which returns a fixed response,
//! `ScriptedLlm` supports scripted tool call sequences for driving
//! end-to-end parity scenarios through the agentic loop.
//!
//! # Usage
//!
//! ```rust,no_run
//! use ironclaw::testing::scripted_llm::{ScriptedLlm, ScriptedStep};
//!
//! let llm = ScriptedLlm::new(vec![
//!     ScriptedStep::tool_calls(vec![("read_file", json!({"path": "f.txt"}))]),
//!     ScriptedStep::text("File contains: hello"),
//! ]);
//! ```

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use rust_decimal::Decimal;

use crate::error::LlmError;
use crate::llm::{
    CompletionRequest, CompletionResponse, FinishReason, LlmProvider, ToolCall,
    ToolCompletionRequest, ToolCompletionResponse,
};

/// A single step in a scripted LLM conversation.
#[derive(Debug, Clone)]
pub enum ScriptedStep {
    /// Return a text-only response (no tool calls).
    Text(String),
    /// Return one or more tool calls.
    ToolCalls {
        tool_calls: Vec<ToolCall>,
        content: Option<String>,
    },
}

impl ScriptedStep {
    /// Create a text-only step.
    pub fn text(s: impl Into<String>) -> Self {
        Self::Text(s.into())
    }

    /// Create a tool-calls step from (name, arguments) pairs.
    ///
    /// Tool call IDs are auto-generated from the step index.
    pub fn tool_calls(calls: Vec<(&str, serde_json::Value)>) -> Self {
        let tool_calls = calls
            .into_iter()
            .enumerate()
            .map(|(i, (name, args))| ToolCall {
                id: format!("tc_{i:04}"),
                name: name.to_string(),
                arguments: args,
                reasoning: None,
            })
            .collect();
        Self::ToolCalls {
            tool_calls,
            content: None,
        }
    }
}

/// A multi-turn LLM stub that returns pre-programmed responses in sequence.
pub struct ScriptedLlm {
    steps: Mutex<Vec<ScriptedStep>>,
    cursor: AtomicUsize,
    model_name: String,
}

impl ScriptedLlm {
    /// Create with a sequence of scripted steps.
    pub fn new(steps: Vec<ScriptedStep>) -> Self {
        Self {
            steps: Mutex::new(steps),
            cursor: AtomicUsize::new(0),
            model_name: "scripted-parity".to_string(),
        }
    }

    /// How many steps have been consumed so far.
    pub fn calls(&self) -> usize {
        self.cursor.load(Ordering::Relaxed)
    }

    fn next_step(&self) -> ScriptedStep {
        let idx = self.cursor.fetch_add(1, Ordering::SeqCst);
        let guard = self.steps.lock().expect("lock poisoned");
        guard.get(idx).cloned().unwrap_or_else(|| {
            panic!(
                "ScriptedLlm: no step at index {idx} (total: {})",
                guard.len()
            )
        })
    }
}

#[async_trait]
impl LlmProvider for ScriptedLlm {
    fn model_name(&self) -> &str {
        &self.model_name
    }

    fn cost_per_token(&self) -> (Decimal, Decimal) {
        (Decimal::ZERO, Decimal::ZERO)
    }

    async fn complete(&self, _request: CompletionRequest) -> Result<CompletionResponse, LlmError> {
        let step = self.next_step();
        let content = match step {
            ScriptedStep::Text(t) => t,
            ScriptedStep::ToolCalls { content, .. } => content.unwrap_or_default(),
        };
        Ok(CompletionResponse {
            content,
            input_tokens: 100,
            output_tokens: 50,
            finish_reason: FinishReason::Stop,
            cache_read_input_tokens: 0,
            cache_creation_input_tokens: 0,
        })
    }

    async fn complete_with_tools(
        &self,
        _request: ToolCompletionRequest,
    ) -> Result<ToolCompletionResponse, LlmError> {
        let step = self.next_step();
        match step {
            ScriptedStep::Text(t) => Ok(ToolCompletionResponse {
                content: Some(t),
                tool_calls: Vec::new(),
                input_tokens: 100,
                output_tokens: 50,
                finish_reason: FinishReason::Stop,
                cache_read_input_tokens: 0,
                cache_creation_input_tokens: 0,
            }),
            ScriptedStep::ToolCalls {
                tool_calls,
                content,
            } => Ok(ToolCompletionResponse {
                content,
                tool_calls,
                input_tokens: 100,
                output_tokens: 50,
                finish_reason: FinishReason::ToolUse,
                cache_read_input_tokens: 0,
                cache_creation_input_tokens: 0,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_scripted_text_then_tools() {
        let llm = ScriptedLlm::new(vec![
            ScriptedStep::text("hello"),
            ScriptedStep::tool_calls(vec![("read_file", serde_json::json!({"path": "f.txt"}))]),
            ScriptedStep::text("done"),
        ]);

        // Step 0: text
        let resp = llm
            .complete_with_tools(ToolCompletionRequest::new(vec![], vec![]))
            .await
            .expect("step 0");
        assert_eq!(resp.content.as_deref(), Some("hello"));
        assert!(resp.tool_calls.is_empty());

        // Step 1: tool call
        let resp = llm
            .complete_with_tools(ToolCompletionRequest::new(vec![], vec![]))
            .await
            .expect("step 1");
        assert_eq!(resp.tool_calls.len(), 1);
        assert_eq!(resp.tool_calls[0].name, "read_file");

        // Step 2: final text
        let resp = llm
            .complete_with_tools(ToolCompletionRequest::new(vec![], vec![]))
            .await
            .expect("step 2");
        assert_eq!(resp.content.as_deref(), Some("done"));
        assert_eq!(llm.calls(), 3);
    }

    #[tokio::test]
    #[should_panic(expected = "no step at index 1")]
    async fn test_scripted_panics_on_exhaustion() {
        let llm = ScriptedLlm::new(vec![ScriptedStep::text("only one")]);
        let _ = llm
            .complete_with_tools(ToolCompletionRequest::new(vec![], vec![]))
            .await;
        // This should panic — no more steps
        let _ = llm
            .complete_with_tools(ToolCompletionRequest::new(vec![], vec![]))
            .await;
    }
}
