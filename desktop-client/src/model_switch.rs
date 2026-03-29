//! 运行时模型/Provider 切换。
//!
//! `ModelSwitchProvider` 包装原始 provider，支持两种切换模式：
//!
//! 1. **同 provider 内切换**（如 qwen-max → qwen-turbo）：
//!    通过 `set_model()` 或 per-request `model_override` 注入。
//!
//! 2. **跨 provider 切换**（如 DashScope → Z.AI）：
//!    通过 `replace_inner()` 替换底层 provider 实例。
//!    调用方负责用正确的 base_url + api_key 构建新 provider。
//!
//! 所有其他 trait 方法完整委托给当前活跃的 inner provider。

use std::sync::{Arc, Mutex, RwLock};

use async_trait::async_trait;
use ironclaw::llm::{
    CompletionRequest, CompletionResponse, LlmProvider, ToolCompletionRequest,
    ToolCompletionResponse,
};
use rust_decimal::Decimal;

/// 运行时模型切换 wrapper。
///
/// `inner` 通过 `Mutex` 保护，支持跨 provider 热替换。
/// 锁持有时间极短（仅 Arc::clone），不会阻塞 async 运行时。
/// `override_source` 用于同 provider 内的 per-request 模型注入。
pub struct ModelSwitchProvider {
    inner: Mutex<Arc<dyn LlmProvider>>,
    /// 共享引用，指向 `AppState.model_override`。
    override_source: Arc<RwLock<Option<String>>>,
}

impl ModelSwitchProvider {
    pub fn new(
        inner: Arc<dyn LlmProvider>,
        override_source: Arc<RwLock<Option<String>>>,
    ) -> Self {
        Self {
            inner: Mutex::new(inner),
            override_source,
        }
    }

    /// 替换底层 provider（跨 provider 切换）。
    ///
    /// 同时清除 model_override，因为新 provider 的模型名
    /// 已经在构建时确定，不需要 per-request 覆盖。
    pub fn replace_inner(&self, new_provider: Arc<dyn LlmProvider>) {
        *self.inner.lock().expect("inner provider lock poisoned") = new_provider;
        if let Ok(mut guard) = self.override_source.write() {
            *guard = None;
        }
    }

    /// 获取当前 inner provider 的 Arc clone。
    /// 锁持有时间仅为 Arc::clone 的开销（纳秒级）。
    fn current_inner(&self) -> Arc<dyn LlmProvider> {
        Arc::clone(&*self.inner.lock().expect("inner provider lock poisoned"))
    }

    fn read_override(&self) -> Option<String> {
        self.override_source.read().ok().and_then(|g| g.clone())
    }
}

#[async_trait]
impl LlmProvider for ModelSwitchProvider {
    fn model_name(&self) -> &str {
        // model_name() 返回 &str，需要 'static 生命周期。
        // 无法从 Mutex 内部借出引用，返回固定值。
        // 实际模型名通过 active_model_name() 获取。
        "dynamic"
    }

    fn cost_per_token(&self) -> (Decimal, Decimal) {
        self.current_inner().cost_per_token()
    }

    fn active_model_name(&self) -> String {
        self.read_override()
            .unwrap_or_else(|| self.current_inner().active_model_name())
    }

    fn set_model(&self, model: &str) -> Result<(), ironclaw::error::LlmError> {
        self.current_inner().set_model(model)
    }

    async fn list_models(&self) -> Result<Vec<String>, ironclaw::error::LlmError> {
        self.current_inner().list_models().await
    }

    fn effective_model_name(&self, requested_model: Option<&str>) -> String {
        self.current_inner().effective_model_name(requested_model)
    }

    fn cache_read_discount(&self) -> Decimal {
        self.current_inner().cache_read_discount()
    }

    fn cache_write_multiplier(&self) -> Decimal {
        self.current_inner().cache_write_multiplier()
    }

    fn calculate_cost(&self, input_tokens: u32, output_tokens: u32) -> Decimal {
        self.current_inner().calculate_cost(input_tokens, output_tokens)
    }

    async fn complete(
        &self,
        mut request: CompletionRequest,
    ) -> Result<CompletionResponse, ironclaw::error::LlmError> {
        if request.model.is_none() {
            request.model = self.read_override();
        }
        let provider = self.current_inner();
        provider.complete(request).await
    }

    async fn complete_with_tools(
        &self,
        mut request: ToolCompletionRequest,
    ) -> Result<ToolCompletionResponse, ironclaw::error::LlmError> {
        if request.model.is_none() {
            request.model = self.read_override();
        }
        let provider = self.current_inner();
        provider.complete_with_tools(request).await
    }
}
