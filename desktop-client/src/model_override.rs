//! 模型切换扩展 — 纯 desktop-client 侧实现，不修改 ironclaw。
//!
//! ironclaw 的 `CompletionRequest` / `ToolCompletionRequest` 都有
//! `model: Option<String>` 字段，provider 实现会读取它来决定使用哪个模型。
//! 但 ironclaw 的 dispatcher 默认不从消息 metadata 读取模型覆盖。
//!
//! `ModelOverrideLlmProvider` 包装原始 provider，在每次 LLM 调用时
//! 从共享状态 `ModelOverrideState` 读取当前选择的模型，并注入到 request。
//!
//! # 数据流
//!
//! ```text
//! 前端选择模型
//!   → send_chat_message(model_id)
//!   → ModelOverrideState::set(model_id)
//!   → Agent 处理消息
//!   → ModelOverrideLlmProvider::complete_with_tools()
//!   → 读取 ModelOverrideState::get() → 注入到 request.model
//!   → 原始 provider 使用指定模型
//! ```

use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use ironclaw::llm::{
    CompletionRequest, CompletionResponse, LlmProvider, ToolCompletionRequest,
    ToolCompletionResponse,
};
use rust_decimal::Decimal;

/// 当前选择的模型 ID 共享状态。
///
/// `None` 表示使用 provider 默认模型（不覆盖）。
/// 使用 `std::sync::RwLock` 而非 `tokio::sync::RwLock`，
/// 使 `set()` / `get()` 保持同步，避免不必要的 async 传染。
#[derive(Debug, Clone, Default)]
pub struct ModelOverrideState(Arc<RwLock<Option<String>>>);

impl ModelOverrideState {
    pub fn new() -> Self {
        Self(Arc::new(RwLock::new(None)))
    }

    /// 设置当前模型覆盖。`None` 表示恢复默认。
    pub fn set(&self, model_id: Option<String>) {
        *self.0.write().expect("model override lock poisoned") = model_id;
    }

    /// 读取当前模型覆盖。
    pub fn get(&self) -> Option<String> {
        self.0.read().expect("model override lock poisoned").clone()
    }
}

/// 包装原始 `LlmProvider`，在每次调用时注入当前选择的模型。
///
/// 利用 `CompletionRequest.model` / `ToolCompletionRequest.model` 字段
/// （ironclaw 原始代码已支持，只是 dispatcher 不设置它），
/// 在 provider 层面透明地注入模型覆盖，不修改 ironclaw 任何代码。
pub struct ModelOverrideLlmProvider {
    inner: Arc<dyn LlmProvider>,
    override_state: ModelOverrideState,
}

impl ModelOverrideLlmProvider {
    pub fn new(inner: Arc<dyn LlmProvider>, override_state: ModelOverrideState) -> Self {
        Self {
            inner,
            override_state,
        }
    }
}

#[async_trait]
impl LlmProvider for ModelOverrideLlmProvider {
    fn model_name(&self) -> &str {
        self.inner.model_name()
    }

    fn cost_per_token(&self) -> (Decimal, Decimal) {
        self.inner.cost_per_token()
    }

    async fn complete(
        &self,
        mut request: CompletionRequest,
    ) -> Result<CompletionResponse, ironclaw::error::LlmError> {
        if request.model.is_none() {
            request.model = self.override_state.get();
        }
        self.inner.complete(request).await
    }

    async fn complete_with_tools(
        &self,
        mut request: ToolCompletionRequest,
    ) -> Result<ToolCompletionResponse, ironclaw::error::LlmError> {
        if request.model.is_none() {
            request.model = self.override_state.get();
        }
        self.inner.complete_with_tools(request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ironclaw::llm::{ChatMessage, FinishReason};
    use std::sync::{Arc, RwLock};

    // ── 测试桩 ────────────────────────────────────────────────────

    /// 捕获每次调用中 request.model 字段的测试桩。
    struct CapturingProvider {
        captured_model: Arc<RwLock<Option<String>>>,
    }

    impl CapturingProvider {
        fn new() -> Self {
            Self {
                captured_model: Arc::new(RwLock::new(None)),
            }
        }

        fn captured_model(&self) -> Option<String> {
            self.captured_model.read().unwrap().clone()
        }
    }

    #[async_trait]
    impl LlmProvider for CapturingProvider {
        fn model_name(&self) -> &str {
            "default-model"
        }

        fn cost_per_token(&self) -> (Decimal, Decimal) {
            (Decimal::ZERO, Decimal::ZERO)
        }

        async fn complete(
            &self,
            request: CompletionRequest,
        ) -> Result<CompletionResponse, ironclaw::error::LlmError> {
            *self.captured_model.write().unwrap() = request.model.clone();
            Ok(CompletionResponse {
                content: "ok".to_string(),
                input_tokens: 1,
                output_tokens: 1,
                finish_reason: FinishReason::Stop,
                cache_read_input_tokens: 0,
                cache_creation_input_tokens: 0,
            })
        }

        async fn complete_with_tools(
            &self,
            request: ToolCompletionRequest,
        ) -> Result<ToolCompletionResponse, ironclaw::error::LlmError> {
            *self.captured_model.write().unwrap() = request.model.clone();
            Ok(ToolCompletionResponse {
                content: Some("ok".to_string()),
                tool_calls: vec![],
                input_tokens: 1,
                output_tokens: 1,
                finish_reason: FinishReason::Stop,
                cache_read_input_tokens: 0,
                cache_creation_input_tokens: 0,
            })
        }
    }

    // =========================================================================
    // 单元测试 — 正常路径
    // =========================================================================

    /// 验证：设置 model override 后，complete() 注入正确的模型。
    #[tokio::test]
    async fn req_model_override_01_injected_into_complete() {
        let inner = Arc::new(CapturingProvider::new());
        let state = ModelOverrideState::new();
        let provider = ModelOverrideLlmProvider::new(inner.clone(), state.clone());

        state.set(Some("gpt-4o".to_string()));

        let request = CompletionRequest::new(vec![ChatMessage::user("hello")]);
        provider.complete(request).await.unwrap();

        assert_eq!(inner.captured_model().as_deref(), Some("gpt-4o"));
    }

    /// 验证：设置 model override 后，complete_with_tools() 注入正确的模型。
    #[tokio::test]
    async fn req_model_override_02_injected_into_complete_with_tools() {
        let inner = Arc::new(CapturingProvider::new());
        let state = ModelOverrideState::new();
        let provider = ModelOverrideLlmProvider::new(inner.clone(), state.clone());

        state.set(Some("claude-3-5-sonnet".to_string()));

        let request = ToolCompletionRequest::new(vec![ChatMessage::user("hello")], vec![]);
        provider.complete_with_tools(request).await.unwrap();

        assert_eq!(inner.captured_model().as_deref(), Some("claude-3-5-sonnet"));
    }

    /// 验证：未设置 override 时，request.model 保持 None（使用 provider 默认）。
    #[tokio::test]
    async fn req_model_override_03_no_override_passes_none() {
        let inner = Arc::new(CapturingProvider::new());
        let provider = ModelOverrideLlmProvider::new(inner.clone(), ModelOverrideState::new());

        let request = CompletionRequest::new(vec![ChatMessage::user("hello")]);
        provider.complete(request).await.unwrap();

        assert!(inner.captured_model().is_none());
    }

    /// 验证：request 已有 model 时，不被 override 覆盖（request 优先）。
    #[tokio::test]
    async fn req_model_override_04_request_model_takes_priority() {
        let inner = Arc::new(CapturingProvider::new());
        let state = ModelOverrideState::new();
        let provider = ModelOverrideLlmProvider::new(inner.clone(), state.clone());

        state.set(Some("gpt-4o".to_string()));

        let request = CompletionRequest::new(vec![ChatMessage::user("hello")])
            .with_model("deepseek-chat");
        provider.complete(request).await.unwrap();

        assert_eq!(inner.captured_model().as_deref(), Some("deepseek-chat"));
    }

    // =========================================================================
    // 失败路径
    // =========================================================================

    /// 验证：override 设为 None 后，恢复使用 provider 默认模型。
    #[tokio::test]
    async fn test_failure_clear_override_restores_default() {
        let inner = Arc::new(CapturingProvider::new());
        let state = ModelOverrideState::new();
        let provider = ModelOverrideLlmProvider::new(inner.clone(), state.clone());

        state.set(Some("gpt-4o".to_string()));
        state.set(None);

        let request = CompletionRequest::new(vec![ChatMessage::user("hello")]);
        provider.complete(request).await.unwrap();

        assert!(inner.captured_model().is_none());
    }

    // =========================================================================
    // 契约测试
    // =========================================================================

    /// 契约测试：ModelOverrideLlmProvider 实现了 LlmProvider trait（编译即验证）。
    #[test]
    fn test_contract_implements_llm_provider() {
        fn assert_llm_provider<T: LlmProvider>() {}
        assert_llm_provider::<ModelOverrideLlmProvider>();
    }

    /// 契约测试：model_name() 委托给内部 provider。
    #[test]
    fn test_contract_model_name_delegates_to_inner() {
        let provider =
            ModelOverrideLlmProvider::new(Arc::new(CapturingProvider::new()), ModelOverrideState::new());
        assert_eq!(provider.model_name(), "default-model");
    }

    /// 契约测试：cost_per_token() 委托给内部 provider。
    #[test]
    fn test_contract_cost_delegates_to_inner() {
        let provider =
            ModelOverrideLlmProvider::new(Arc::new(CapturingProvider::new()), ModelOverrideState::new());
        assert_eq!(provider.cost_per_token(), (Decimal::ZERO, Decimal::ZERO));
    }

    // =========================================================================
    // 安全审计
    // =========================================================================

    /// 安全审计：并发读写 override 不会 panic（RwLock 保证）。
    #[tokio::test]
    async fn test_audit_concurrent_set_no_panic() {
        let state = ModelOverrideState::new();
        let state_clone = state.clone();

        let h1 = tokio::spawn(async move {
            for i in 0..100 {
                state.set(Some(format!("model-{}", i)));
            }
        });
        let h2 = tokio::spawn(async move {
            for _ in 0..100 {
                let _ = state_clone.get();
            }
        });

        h1.await.unwrap();
        h2.await.unwrap();
    }
}
