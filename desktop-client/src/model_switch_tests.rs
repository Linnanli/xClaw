//! ModelSwitchProvider 单元测试。

#[cfg(test)]
mod tests {
    use std::sync::{Arc, RwLock};

    use async_trait::async_trait;
    use ironclaw::llm::{
        ChatMessage, CompletionRequest, CompletionResponse, FinishReason, LlmProvider,
        ToolCompletionRequest, ToolCompletionResponse,
    };
    use rust_decimal::Decimal;

    use crate::model_switch::ModelSwitchProvider;

    /// 捕获 request.model 的测试桩。
    struct CapturingProvider {
        captured: Arc<RwLock<Option<String>>>,
    }

    impl CapturingProvider {
        fn new() -> Self {
            Self { captured: Arc::new(RwLock::new(None)) }
        }
        fn captured_model(&self) -> Option<String> {
            self.captured.read().unwrap().clone()
        }
    }

    #[async_trait]
    impl LlmProvider for CapturingProvider {
        fn model_name(&self) -> &str { "test-model" }
        fn cost_per_token(&self) -> (Decimal, Decimal) { (Decimal::ZERO, Decimal::ZERO) }
        async fn complete(&self, req: CompletionRequest) -> Result<CompletionResponse, ironclaw::error::LlmError> {
            *self.captured.write().unwrap() = req.model.clone();
            Ok(CompletionResponse {
                content: "ok".into(), input_tokens: 1, output_tokens: 1,
                finish_reason: FinishReason::Stop,
                cache_read_input_tokens: 0, cache_creation_input_tokens: 0,
            })
        }
        async fn complete_with_tools(&self, req: ToolCompletionRequest) -> Result<ToolCompletionResponse, ironclaw::error::LlmError> {
            *self.captured.write().unwrap() = req.model.clone();
            Ok(ToolCompletionResponse {
                content: Some("ok".into()), tool_calls: vec![], input_tokens: 1, output_tokens: 1,
                finish_reason: FinishReason::Stop,
                cache_read_input_tokens: 0, cache_creation_input_tokens: 0,
            })
        }
    }

    // ── 正常路径 ──

    #[tokio::test]
    async fn req_model_switch_01_override_injected_into_complete() {
        let inner = Arc::new(CapturingProvider::new());
        let ovr = Arc::new(RwLock::new(Some("glm-4.7-flash".to_string())));
        let provider = ModelSwitchProvider::new(inner.clone(), ovr);

        let req = CompletionRequest::new(vec![ChatMessage::user("hi")]);
        provider.complete(req).await.unwrap();
        assert_eq!(inner.captured_model().as_deref(), Some("glm-4.7-flash"));
    }

    #[tokio::test]
    async fn req_model_switch_02_override_injected_into_complete_with_tools() {
        let inner = Arc::new(CapturingProvider::new());
        let ovr = Arc::new(RwLock::new(Some("glm-4.7-flash".to_string())));
        let provider = ModelSwitchProvider::new(inner.clone(), ovr);

        let req = ToolCompletionRequest::new(vec![ChatMessage::user("hi")], vec![]);
        provider.complete_with_tools(req).await.unwrap();
        assert_eq!(inner.captured_model().as_deref(), Some("glm-4.7-flash"));
    }

    #[tokio::test]
    async fn req_model_switch_03_no_override_passes_none() {
        let inner = Arc::new(CapturingProvider::new());
        let ovr = Arc::new(RwLock::new(None));
        let provider = ModelSwitchProvider::new(inner.clone(), ovr);

        let req = CompletionRequest::new(vec![ChatMessage::user("hi")]);
        provider.complete(req).await.unwrap();
        assert!(inner.captured_model().is_none());
    }

    #[tokio::test]
    async fn req_model_switch_04_request_model_takes_priority() {
        let inner = Arc::new(CapturingProvider::new());
        let ovr = Arc::new(RwLock::new(Some("glm-4.7-flash".to_string())));
        let provider = ModelSwitchProvider::new(inner.clone(), ovr);

        let req = CompletionRequest::new(vec![ChatMessage::user("hi")])
            .with_model("explicit-model");
        provider.complete(req).await.unwrap();
        assert_eq!(inner.captured_model().as_deref(), Some("explicit-model"));
    }

    // ── 失败路径 ──

    #[tokio::test]
    async fn test_failure_clear_override_restores_default() {
        let inner = Arc::new(CapturingProvider::new());
        let ovr = Arc::new(RwLock::new(Some("glm-4.7-flash".to_string())));
        let provider = ModelSwitchProvider::new(inner.clone(), Arc::clone(&ovr));

        // 清除 override
        *ovr.write().unwrap() = None;

        let req = CompletionRequest::new(vec![ChatMessage::user("hi")]);
        provider.complete(req).await.unwrap();
        assert!(inner.captured_model().is_none());
    }

    // ── 契约测试 ──

    #[test]
    fn test_contract_implements_llm_provider() {
        fn assert_llm_provider<T: LlmProvider + ?Sized>() {}
        assert_llm_provider::<ModelSwitchProvider>();
    }

    #[test]
    fn test_contract_delegates_model_name() {
        let inner = Arc::new(CapturingProvider::new());
        let provider = ModelSwitchProvider::new(inner, Arc::new(RwLock::new(None)));
        assert_eq!(provider.model_name(), "test-model");
    }

    #[test]
    fn test_contract_active_model_name_reflects_override() {
        let inner = Arc::new(CapturingProvider::new());
        let ovr = Arc::new(RwLock::new(Some("glm-4.7-flash".to_string())));
        let provider = ModelSwitchProvider::new(inner, ovr);
        assert_eq!(provider.active_model_name(), "glm-4.7-flash");
    }

    #[test]
    fn test_contract_active_model_name_delegates_when_no_override() {
        let inner = Arc::new(CapturingProvider::new());
        let provider = ModelSwitchProvider::new(inner, Arc::new(RwLock::new(None)));
        assert_eq!(provider.active_model_name(), "test-model");
    }
}
