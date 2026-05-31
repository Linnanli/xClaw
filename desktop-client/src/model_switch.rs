//! 运行时模型/Provider 切换。
//!
//! `ModelSwitchProvider` 是 desktop 端业务壳，真正的"原子替换 inner"
//! 由 [`SwappableLlmProvider`] 负责（在 `dasclaw_llm_provider` crate）。
//! 本壳只保留 desktop 专属的 `override_source`（来自 `AppState.model_override`），
//! 用于在 `request.model` 缺失时注入 per-request 模型名。
//!
//! 两种切换模式：
//!
//! 1. **同 provider 内切换**（如 qwen-max → qwen-turbo）：
//!    `set_model()` 透传到底层；或通过 `model_override` 注入 per-request。
//!
//! 2. **跨 provider 切换**（如 DashScope → Z.AI）：
//!    `replace_inner()` 调用 [`SwappableLlmProvider::swap`] 原子替换。

use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use ironclaw::llm::{
    CompletionRequest, CompletionResponse, LlmProvider, SwappableLlmProvider,
    ToolCompletionRequest, ToolCompletionResponse,
};
use rust_decimal::Decimal;

/// Desktop 端模型切换 wrapper：薄壳委托 + override 注入。
pub struct ModelSwitchProvider {
    inner: Arc<SwappableLlmProvider>,
    /// 共享引用，指向 `AppState.model_override`。
    override_source: Arc<RwLock<Option<String>>>,
}

impl ModelSwitchProvider {
    pub fn new(inner: Arc<dyn LlmProvider>, override_source: Arc<RwLock<Option<String>>>) -> Self {
        Self {
            inner: Arc::new(SwappableLlmProvider::new(inner)),
            override_source,
        }
    }

    /// 替换底层 provider（跨 provider 切换）。
    ///
    /// 同时清除 model_override，因为新 provider 的模型名
    /// 已经在构建时确定，不需要 per-request 覆盖。
    pub fn replace_inner(&self, new_provider: Arc<dyn LlmProvider>) {
        self.inner.swap(new_provider);
        if let Ok(mut guard) = self.override_source.write() {
            *guard = None;
        }
    }

    /// 暴露底层 [`SwappableLlmProvider`] handle，供未来通过
    /// [`ironclaw::llm::LlmReloadHandle`] 接入热重载链路。
    pub fn handle(&self) -> Arc<SwappableLlmProvider> {
        Arc::clone(&self.inner)
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
