//! Per-request 模型注入 — `set_model()` 不支持时的回退机制。
//!
//! 部分 LLM provider（如 rig-core）不支持运行时 `set_model()`。
//! `ModelSwitchProvider` 包装原始 provider，在每次 LLM 调用时
//! 从共享的 `model_override`（`Arc<RwLock>`）读取目标模型，
//! 注入到 `request.model`。所有其他 trait 方法完整委托。

use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use ironclaw::llm::{
    CompletionRequest, CompletionResponse, LlmProvider, ToolCompletionRequest,
    ToolCompletionResponse,
};
use rust_decimal::Decimal;

/// 轻量 wrapper：per-request 模型注入。
pub struct ModelSwitchProvider {
    inner: Arc<dyn LlmProvider>,
    /// 共享引用，指向 `AppState.model_override`。
    override_source: Arc<RwLock<Option<String>>>,
}

impl ModelSwitchProvider {
    pub fn new(
        inner: Arc<dyn LlmProvider>,
        override_source: Arc<RwLock<Option<String>>>,
    ) -> Self {
        Self { inner, override_source }
    }

    fn read_override(&self) -> Option<String> {
        self.override_source.read().ok().and_then(|g| g.clone())
    }
}

#[async_trait]
impl LlmProvider for ModelSwitchProvider {
    fn model_name(&self) -> &str {
        self.inner.model_name()
    }

    fn cost_per_token(&self) -> (Decimal, Decimal) {
        self.inner.cost_per_token()
    }

    fn active_model_name(&self) -> String {
        self.read_override()
            .unwrap_or_else(|| self.inner.active_model_name())
    }

    fn set_model(&self, model: &str) -> Result<(), ironclaw::error::LlmError> {
        self.inner.set_model(model)
    }

    async fn list_models(&self) -> Result<Vec<String>, ironclaw::error::LlmError> {
        self.inner.list_models().await
    }

    fn effective_model_name(&self, requested_model: Option<&str>) -> String {
        self.inner.effective_model_name(requested_model)
    }

    fn cache_read_discount(&self) -> Decimal {
        self.inner.cache_read_discount()
    }

    fn cache_write_multiplier(&self) -> Decimal {
        self.inner.cache_write_multiplier()
    }

    fn calculate_cost(&self, input_tokens: u32, output_tokens: u32) -> Decimal {
        self.inner.calculate_cost(input_tokens, output_tokens)
    }

    async fn complete(
        &self,
        mut request: CompletionRequest,
    ) -> Result<CompletionResponse, ironclaw::error::LlmError> {
        if request.model.is_none() {
            request.model = self.read_override();
        }
        self.inner.complete(request).await
    }

    async fn complete_with_tools(
        &self,
        mut request: ToolCompletionRequest,
    ) -> Result<ToolCompletionResponse, ironclaw::error::LlmError> {
        if request.model.is_none() {
            request.model = self.read_override();
        }
        self.inner.complete_with_tools(request).await
    }
}
