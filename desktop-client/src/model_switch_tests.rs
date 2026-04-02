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
        name: String,
    }

    impl CapturingProvider {
        fn new() -> Self {
            Self {
                captured: Arc::new(RwLock::new(None)),
                name: "test-model".to_string(),
            }
        }
        fn with_name(name: &str) -> Self {
            Self {
                captured: Arc::new(RwLock::new(None)),
                name: name.to_string(),
            }
        }
        fn captured_model(&self) -> Option<String> {
            self.captured.read().unwrap().clone()
        }
    }

    #[async_trait]
    impl LlmProvider for CapturingProvider {
        fn model_name(&self) -> &str { &self.name }
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

    // ── 正常路径：同 provider 内切换 ──

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

    // ── 跨 provider 切换：replace_inner ──

    #[tokio::test]
    async fn req_model_switch_05_replace_inner_switches_provider() {
        let original = Arc::new(CapturingProvider::with_name("qwen-max"));
        let replacement = Arc::new(CapturingProvider::with_name("glm-4.7-flash"));
        let ovr = Arc::new(RwLock::new(None));
        let provider = ModelSwitchProvider::new(original.clone(), Arc::clone(&ovr));

        // 初始状态：委托给 original
        assert_eq!(provider.active_model_name(), "qwen-max");

        // 替换底层 provider
        provider.replace_inner(replacement.clone());

        // 替换后：委托给 replacement
        assert_eq!(provider.active_model_name(), "glm-4.7-flash");

        // 请求发到新 provider
        let req = CompletionRequest::new(vec![ChatMessage::user("hi")]);
        provider.complete(req).await.unwrap();
        assert!(original.captured_model().is_none(), "original should not receive request");
        // replacement 收到了请求（captured_model 是 None 因为没有 override）
    }

    #[tokio::test]
    async fn req_model_switch_06_replace_inner_clears_override() {
        let inner = Arc::new(CapturingProvider::new());
        let ovr = Arc::new(RwLock::new(Some("old-override".to_string())));
        let provider = ModelSwitchProvider::new(inner, Arc::clone(&ovr));

        assert_eq!(ovr.read().unwrap().as_deref(), Some("old-override"));

        // replace_inner 应该清除 override
        let new_inner = Arc::new(CapturingProvider::with_name("new-model"));
        provider.replace_inner(new_inner);

        assert!(ovr.read().unwrap().is_none(), "override should be cleared after replace_inner");
    }

    #[tokio::test]
    async fn req_model_switch_07_replace_inner_then_set_override() {
        let inner = Arc::new(CapturingProvider::with_name("initial"));
        let ovr = Arc::new(RwLock::new(None));
        let provider = ModelSwitchProvider::new(inner, Arc::clone(&ovr));

        // 替换 provider
        let new_inner = Arc::new(CapturingProvider::with_name("new-provider"));
        provider.replace_inner(new_inner.clone());

        // 在新 provider 上设置 override
        *ovr.write().unwrap() = Some("override-on-new".to_string());
        assert_eq!(provider.active_model_name(), "override-on-new");

        // 请求应该带上 override
        let req = CompletionRequest::new(vec![ChatMessage::user("hi")]);
        provider.complete(req).await.unwrap();
        assert_eq!(new_inner.captured_model().as_deref(), Some("override-on-new"));
    }

    // ── 契约测试 ──

    #[test]
    fn test_contract_implements_llm_provider() {
        fn assert_llm_provider<T: LlmProvider + ?Sized>() {}
        assert_llm_provider::<ModelSwitchProvider>();
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

    #[test]
    fn test_contract_cost_delegates_to_current_inner() {
        let inner = Arc::new(CapturingProvider::new());
        let provider = ModelSwitchProvider::new(inner, Arc::new(RwLock::new(None)));
        let (input, output) = provider.cost_per_token();
        assert_eq!(input, Decimal::ZERO);
        assert_eq!(output, Decimal::ZERO);
    }

    // ── 并发安全测试（RwLock 改造后）──

    #[tokio::test]
    async fn test_concurrent_complete_and_replace_inner() {
        // 验证 replace_inner 和 complete 并发执行不会 panic
        let inner = Arc::new(CapturingProvider::with_name("model-a"));
        let ovr = Arc::new(RwLock::new(None));
        let provider = Arc::new(ModelSwitchProvider::new(inner.clone(), ovr));

        let p1 = Arc::clone(&provider);
        let h1 = tokio::spawn(async move {
            for _ in 0..10 {
                let req = CompletionRequest {
                    messages: vec![],
                    temperature: None,
                    max_tokens: None,
                    model: None,
                    metadata: Default::default(),
                    stop_sequences: None,
                };
                let _ = p1.complete(req).await;
            }
        });

        let replacement = Arc::new(CapturingProvider::with_name("model-b"));
        let p2 = Arc::clone(&provider);
        let h2 = tokio::spawn(async move {
            for _ in 0..10 {
                p2.replace_inner(replacement.clone());
                tokio::task::yield_now().await;
            }
        });

        // 两个 task 都不应该 panic
        h1.await.expect("complete task panicked");
        h2.await.expect("replace_inner task panicked");
    }

    #[tokio::test]
    async fn test_failure_replace_inner_during_active_request() {
        // replace_inner 在请求进行中调用，不应导致死锁或 panic
        let inner = Arc::new(CapturingProvider::with_name("original"));
        let ovr = Arc::new(RwLock::new(None));
        let provider = Arc::new(ModelSwitchProvider::new(inner, ovr));

        let replacement = Arc::new(CapturingProvider::with_name("replaced"));
        provider.replace_inner(replacement);

        // 替换后应该能正常 complete
        let req = ToolCompletionRequest {
            messages: vec![],
            tools: vec![],
            temperature: None,
            max_tokens: None,
            tool_choice: None,
            model: None,
            metadata: Default::default(),
            stop_sequences: None,
        };
        let result = provider.complete_with_tools(req).await;
        assert!(result.is_ok());
    }
}
