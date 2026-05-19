//! Test-only stub LLM provider, used by this crate's internal unit tests
//! for circuit breaker / retry / response cache / smart routing / reasoning.
//!
//! This is **not** a public API surface for the embedding application: a
//! richer harness lives in `ironclaw::testing` (which also wires up channels,
//! databases, and tool registries). We keep a minimal mirror here so the
//! provider crate's unit tests can run standalone, without pulling ironclaw
//! into the provider crate's dependency graph.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use async_trait::async_trait;
use rust_decimal::Decimal;

use crate::provider::error::LlmError;
use crate::provider::provider::{
    CompletionRequest, CompletionResponse, FinishReason, LlmProvider, ToolCompletionRequest,
    ToolCompletionResponse,
};

#[derive(Copy, Clone, Debug)]
enum StubErrorKind {
    Transient,
    NonTransient,
}

/// Minimal stub `LlmProvider` for unit tests.
///
/// Mirrors the public surface of `ironclaw::testing::StubLlm` that the
/// provider-internal tests rely on (`new`, `failing`, `failing_non_transient`,
/// `with_model_name`, `set_failing`, `calls`, and the public `call_count`
/// atomic). Behavior toggles (fault injection, channel wiring) are
/// intentionally absent — those concerns live with the application-level
/// harness in ironclaw.
pub struct StubLlm {
    model_name: String,
    response: String,
    pub call_count: AtomicU32,
    should_fail: AtomicBool,
    error_kind: StubErrorKind,
}

impl StubLlm {
    pub fn new(response: impl Into<String>) -> Self {
        Self {
            model_name: "stub-model".to_string(),
            response: response.into(),
            call_count: AtomicU32::new(0),
            should_fail: AtomicBool::new(false),
            error_kind: StubErrorKind::Transient,
        }
    }

    pub fn failing(name: impl Into<String>) -> Self {
        Self {
            model_name: name.into(),
            response: String::new(),
            call_count: AtomicU32::new(0),
            should_fail: AtomicBool::new(true),
            error_kind: StubErrorKind::Transient,
        }
    }

    pub fn failing_non_transient(name: impl Into<String>) -> Self {
        Self {
            model_name: name.into(),
            response: String::new(),
            call_count: AtomicU32::new(0),
            should_fail: AtomicBool::new(true),
            error_kind: StubErrorKind::NonTransient,
        }
    }

    pub fn with_model_name(mut self, name: impl Into<String>) -> Self {
        self.model_name = name.into();
        self
    }

    pub fn calls(&self) -> u32 {
        self.call_count.load(Ordering::Relaxed)
    }

    pub fn set_failing(&self, fail: bool) {
        self.should_fail.store(fail, Ordering::Relaxed);
    }

    fn make_error(&self) -> LlmError {
        match self.error_kind {
            StubErrorKind::Transient => LlmError::RequestFailed {
                provider: self.model_name.clone(),
                reason: "server error".to_string(),
            },
            StubErrorKind::NonTransient => LlmError::ContextLengthExceeded {
                used: 100_000,
                limit: 50_000,
            },
        }
    }
}

impl Default for StubLlm {
    fn default() -> Self {
        Self::new("OK")
    }
}

/// Helper for cheaply constructing a shared stub.
pub fn stub(response: impl Into<String>) -> Arc<StubLlm> {
    Arc::new(StubLlm::new(response))
}

#[async_trait]
impl LlmProvider for StubLlm {
    fn model_name(&self) -> &str {
        &self.model_name
    }

    fn cost_per_token(&self) -> (Decimal, Decimal) {
        (Decimal::ZERO, Decimal::ZERO)
    }

    async fn complete(&self, _request: CompletionRequest) -> Result<CompletionResponse, LlmError> {
        self.call_count.fetch_add(1, Ordering::Relaxed);
        if self.should_fail.load(Ordering::Relaxed) {
            return Err(self.make_error());
        }
        Ok(CompletionResponse {
            content: self.response.clone(),
            input_tokens: 10,
            output_tokens: 5,
            finish_reason: FinishReason::Stop,
            cache_read_input_tokens: 0,
            cache_creation_input_tokens: 0,
        })
    }

    async fn complete_with_tools(
        &self,
        _request: ToolCompletionRequest,
    ) -> Result<ToolCompletionResponse, LlmError> {
        self.call_count.fetch_add(1, Ordering::Relaxed);
        if self.should_fail.load(Ordering::Relaxed) {
            return Err(self.make_error());
        }
        Ok(ToolCompletionResponse {
            content: Some(self.response.clone()),
            tool_calls: Vec::new(),
            input_tokens: 10,
            output_tokens: 5,
            finish_reason: FinishReason::Stop,
            cache_read_input_tokens: 0,
            cache_creation_input_tokens: 0,
        })
    }
}
