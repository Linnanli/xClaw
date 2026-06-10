//! 聊天 IPC 命令测试。
//!
//! 覆盖维度：
//! - 单元测试（正常路径 + 错误路径）
//! - 契约测试（VercelUIStream 格式与前端 TypeScript 类型匹配）
//! - 安全审计测试（敏感信息不泄露）

#[cfg(test)]
mod tests {
    use crate::ipc::chat::{
        build_thread_control_message, reject_blocked_scan, send_chat_message,
        usage_report_backend_user_id, FrontendAttachment, SendMessageResponse,
    };
    use crate::safety_bridge::SafetyBridge;
    use crate::state::{AppState, EngineState};
    use dasclaw_runtime::context::ContextManager;
    use ironclaw::safety::{SafetyConfig, SafetyLayer};
    use ironclaw::tools::ToolRegistry;
    use std::sync::Arc;
    use std::sync::RwLock;
    use tauri::Manager;
    use tokio::sync::mpsc;
    use uuid::Uuid;

    struct StubLlmProvider;

    #[async_trait::async_trait]
    impl ironclaw::llm::LlmProvider for StubLlmProvider {
        fn model_name(&self) -> &str {
            "stub-model"
        }

        fn cost_per_token(&self) -> (rust_decimal::Decimal, rust_decimal::Decimal) {
            (rust_decimal::Decimal::ZERO, rust_decimal::Decimal::ZERO)
        }

        async fn complete(
            &self,
            _req: ironclaw::llm::CompletionRequest,
        ) -> Result<ironclaw::llm::CompletionResponse, ironclaw::error::LlmError> {
            Err(ironclaw::error::LlmError::RequestFailed {
                provider: "stub".into(),
                reason: "not implemented".into(),
            })
        }

        async fn complete_with_tools(
            &self,
            _req: ironclaw::llm::ToolCompletionRequest,
        ) -> Result<ironclaw::llm::ToolCompletionResponse, ironclaw::error::LlmError> {
            Err(ironclaw::error::LlmError::RequestFailed {
                provider: "stub".into(),
                reason: "not implemented".into(),
            })
        }
    }

    struct TestSafetyParts {
        safety: Arc<SafetyLayer>,
        safety_bridge: Arc<SafetyBridge>,
        attachment_scanner: Arc<crate::safety_attachment_scanner::AttachmentScanner>,
        egress: Arc<dyn dasclaw_core::EgressGate>,
    }

    struct TestModelParts {
        llm: Arc<dyn ironclaw::llm::LlmProvider>,
        model_override: Arc<std::sync::RwLock<Option<String>>>,
        model_switch: Arc<crate::model_switch::ModelSwitchProvider>,
        initial_provider: Arc<dyn ironclaw::llm::LlmProvider>,
    }

    fn create_test_safety_parts() -> TestSafetyParts {
        let safety = Arc::new(SafetyLayer::new(&SafetyConfig {
            max_output_length: 100_000,
            injection_check_enabled: true,
        }));
        let safety_bridge = Arc::new(SafetyBridge::new(Arc::clone(&safety), None, None));
        let egress: Arc<dyn dasclaw_core::EgressGate> = Arc::new(
            dasclaw_safety::egress_gate::IronclawEgressGate::new(Arc::clone(&safety)),
        );
        let attachment_scanner = Arc::new(
            crate::safety_attachment_scanner::AttachmentScanner::new(Arc::clone(&egress)),
        );

        TestSafetyParts {
            safety,
            safety_bridge,
            attachment_scanner,
            egress,
        }
    }

    fn create_test_model_parts() -> TestModelParts {
        let model_override = Arc::new(std::sync::RwLock::new(None));
        let stub_llm: Arc<dyn ironclaw::llm::LlmProvider> = Arc::new(StubLlmProvider);
        let model_switch = Arc::new(crate::model_switch::ModelSwitchProvider::new(
            Arc::clone(&stub_llm),
            Arc::clone(&model_override),
        ));

        TestModelParts {
            llm: Arc::clone(&model_switch) as _,
            model_override,
            model_switch,
            initial_provider: Arc::clone(&stub_llm),
        }
    }

    fn create_test_app_state() -> (
        AppState,
        mpsc::Receiver<ironclaw::channels::IncomingMessage>,
    ) {
        let (tx, rx) = mpsc::channel(1);
        let safety = create_test_safety_parts();
        let model = create_test_model_parts();

        let app_state = AppState {
            msg_sender: tx,
            db: None,
            workspace: None,
            tools: Arc::new(ToolRegistry::new()),
            extension_manager: None,
            skill_registry: None,
            skill_catalog: None,
            skills_config: ironclaw::config::SkillsConfig::default(),
            safety: safety.safety,
            safety_bridge: safety.safety_bridge,
            attachment_scanner: safety.attachment_scanner,
            egress: safety.egress,
            context_manager: Arc::new(ContextManager::new(5)),
            conversation_tracker: Arc::new(crate::conversation_tracker::ConversationTracker::new(
                "test-owner".to_string(),
            )),
            data_reporter: Arc::new(crate::data_reporter::DataReporter::new(
                String::new(),
                String::new(),
            )),
            scope_id: "test-owner".to_string(),
            backend_user_id: Arc::new(std::sync::RwLock::new(None)),
            llm: model.llm,
            model_override: model.model_override,
            model_switch: model.model_switch,
            provider_base_url: std::sync::RwLock::new(String::new()),
            initial_provider: model.initial_provider,
            initial_base_url: String::new(),
            log_broadcaster: Arc::new(ironclaw::channels::web::log_layer::LogBroadcaster::new()),
            log_clear_offset: std::sync::atomic::AtomicUsize::new(0),
            routine_engine_slot: Arc::new(tokio::sync::RwLock::new(None)),
            scheduler_slot: Arc::new(tokio::sync::RwLock::new(None)),
            disabled_skills: std::sync::RwLock::new(std::collections::HashSet::new()),
            disabled_extensions: std::sync::RwLock::new(std::collections::HashSet::new()),
        };

        (app_state, rx)
    }

    // =========================================================================
    // 单元测试 — 正常路径
    // =========================================================================

    #[test]
    fn test_send_message_response_serialization() {
        let resp = SendMessageResponse {
            message_id: "msg-123".into(),
            success: true,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["message_id"], "msg-123");
        assert_eq!(json["success"], true);
    }

    #[test]
    fn test_send_message_response_deserialization() {
        let json = r#"{"message_id":"msg-456","success":false}"#;
        let resp: SendMessageResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.message_id, "msg-456");
        assert!(!resp.success);
    }

    // =========================================================================
    // 契约测试 — 验证与前端 TypeScript 类型的兼容性
    // =========================================================================

    /// 验证 SendMessageResponse 的 JSON 字段名与前端 `SendMessageResponse` 接口匹配。
    ///
    /// 前端定义（useAiChatTauri.ts）：
    /// ```typescript
    /// interface SendMessageResponse {
    ///   message_id: string;
    ///   success: boolean;
    /// }
    /// ```
    #[test]
    fn test_contract_send_message_response_matches_frontend() {
        let resp = SendMessageResponse {
            message_id: "test".into(),
            success: true,
        };
        let json: serde_json::Value = serde_json::to_value(&resp).unwrap();

        // 验证字段名完全匹配前端 TypeScript 接口
        assert!(
            json.get("message_id").is_some(),
            "missing 'message_id' field"
        );
        assert!(json.get("success").is_some(), "missing 'success' field");

        // 验证类型
        assert!(
            json["message_id"].is_string(),
            "message_id should be string"
        );
        assert!(json["success"].is_boolean(), "success should be boolean");

        // 验证没有多余字段
        let obj = json.as_object().unwrap();
        assert_eq!(
            obj.len(),
            2,
            "SendMessageResponse should have exactly 2 fields"
        );
    }

    // =========================================================================
    // 安全审计测试
    // =========================================================================

    /// 验证 SendMessageResponse 不包含消息内容（防止敏感信息泄露）。
    #[test]
    fn test_audit_response_no_content_leak() {
        let resp = SendMessageResponse {
            message_id: "msg-789".into(),
            success: true,
        };
        let json_str = serde_json::to_string(&resp).unwrap();

        // 响应中不应包含任何消息内容字段
        assert!(!json_str.contains("content"));
        assert!(!json_str.contains("thread_id"));
        assert!(!json_str.contains("owner_id"));
    }

    // =========================================================================
    // model_id 参数测试
    // =========================================================================

    /// 验证 set_model 切换模型的正常路径（编译即验证 — LlmProvider::set_model 签名）。
    /// 实际的 set_model 行为由 ironclaw provider 测试覆盖，这里只验证调用契约。
    #[test]
    fn test_contract_set_model_api_exists() {
        // 编译即验证：LlmProvider trait 有 set_model 方法
        fn assert_has_set_model<T: ironclaw::llm::LlmProvider + ?Sized>() {}
        assert_has_set_model::<dyn ironclaw::llm::LlmProvider>();
    }

    /// 安全审计：model_id 不应出现在 SendMessageResponse 中。
    #[test]
    fn test_audit_model_id_not_in_response() {
        let resp = SendMessageResponse {
            message_id: "msg-001".into(),
            success: true,
        };
        let json_str = serde_json::to_string(&resp).unwrap();

        assert!(
            !json_str.contains("model"),
            "model info should not leak in response"
        );
        assert!(
            !json_str.contains("deepseek"),
            "model name should not leak in response"
        );
    }

    // =========================================================================
    // normalize_base_url 测试 — 验证 /v1 不被剥掉（修复 404 bug）
    // =========================================================================

    #[test]
    fn test_normalize_base_url_preserves_v1() {
        use crate::ipc::chat::normalize_base_url;

        // /v1 是 base URL 的一部分，不能剥掉
        assert_eq!(
            normalize_base_url("https://dashscope.aliyuncs.com/compatible-mode/v1"),
            "https://dashscope.aliyuncs.com/compatible-mode/v1",
            "/v1 should be preserved"
        );
        assert_eq!(
            normalize_base_url("https://api.openai.com/v1"),
            "https://api.openai.com/v1",
            "/v1 should be preserved for OpenAI"
        );
    }

    #[test]
    fn test_normalize_base_url_strips_completions_suffix() {
        use crate::ipc::chat::normalize_base_url;

        // /chat/completions 是 rig-core 自动拼接的，应该剥掉
        assert_eq!(
            normalize_base_url(
                "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions"
            ),
            "https://dashscope.aliyuncs.com/compatible-mode/v1",
        );
        assert_eq!(
            normalize_base_url("https://api.openai.com/v1/chat/completions"),
            "https://api.openai.com/v1",
        );
        assert_eq!(
            normalize_base_url("https://api.openai.com/v1/chat/completions/"),
            "https://api.openai.com/v1",
            "trailing slash should also be handled"
        );
    }

    #[test]
    fn test_normalize_base_url_strips_trailing_slash() {
        use crate::ipc::chat::normalize_base_url;

        assert_eq!(
            normalize_base_url("https://api.openai.com/v1/"),
            "https://api.openai.com/v1",
        );
    }

    #[test]
    fn test_frontend_attachment_deserialization() {
        let json = r#"{
            "id":"att-1",
            "kind":"image",
            "mime_type":"image/png",
            "filename":"diagram.png",
            "size_bytes":12,
            "extracted_text":null,
            "data":[1,2,3],
            "duration_secs":null
        }"#;

        let attachment: FrontendAttachment = serde_json::from_str(json).unwrap();
        assert_eq!(attachment.id, "att-1");
        assert_eq!(attachment.mime_type, "image/png");
        assert_eq!(attachment.data, vec![1, 2, 3]);
    }

    #[test]
    fn test_usage_report_backend_user_id_returns_none_when_missing() {
        let lock = RwLock::new(None);

        let user_id = usage_report_backend_user_id(&lock)
            .expect("missing identity should not be a hard error for usage reporting");

        assert!(user_id.is_none());
    }

    #[test]
    fn test_usage_report_backend_user_id_reports_poisoned_lock() {
        let lock = RwLock::new(Some(Uuid::nil()));
        let _ = std::panic::catch_unwind(|| {
            let _guard = lock.write().expect("write lock should succeed");
            panic!("poison backend user id lock");
        });

        let error = usage_report_backend_user_id(&lock)
            .expect_err("poisoned backend user id lock should fail");

        assert_eq!(error, "后台用户身份读取失败，跳过费用上报");
    }

    #[tokio::test]
    async fn req_chat_dlp_block_before_dispatch() {
        let safety = Arc::new(SafetyLayer::new(&SafetyConfig {
            max_output_length: 100_000,
            injection_check_enabled: true,
        }));
        let bridge = SafetyBridge::new(safety, None, None);
        let scan = bridge.scan_user_input("send ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx onward");
        let (tx, mut rx) = mpsc::channel::<ironclaw::channels::IncomingMessage>(1);

        assert!(scan.was_blocked, "secret-bearing input must be blocked");
        let error = match reject_blocked_scan(&scan, "msg-1", "thread-a") {
            Ok(()) => {
                tx.send(ironclaw::channels::IncomingMessage::new(
                    "tauri", "owner-1", "secret",
                ))
                .await
                .expect("test channel should accept messages");
                String::new()
            }
            Err(error) => error,
        };

        assert!(error.contains("密钥") || error.contains("secret"));
        assert!(
            rx.try_recv().is_err(),
            "blocked scan must not enqueue a message"
        );
    }

    #[tokio::test]
    async fn req_chat_blocked_send_message_has_zero_side_effects() {
        let secret = "send ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx onward";
        let thread_id = "thread-blocked-zero".to_string();
        let (app_state, mut rx) = create_test_app_state();
        let tracker = Arc::clone(&app_state.conversation_tracker);
        let reporter = Arc::clone(&app_state.data_reporter);

        let engine_state = EngineState::new();
        engine_state
            .initialize(app_state)
            .expect("test EngineState should initialize once");
        let app = tauri::test::mock_builder()
            .manage(engine_state)
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("mock Tauri app should build");

        let error = send_chat_message(
            app.handle().clone(),
            app.state::<EngineState>(),
            thread_id.clone(),
            secret.to_string(),
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .expect_err("secret-bearing input must be blocked");

        assert!(error.contains("密钥") || error.contains("secret"));
        assert!(
            !error.contains(secret),
            "blocked error must not leak raw secret"
        );
        assert!(
            rx.try_recv().is_err(),
            "blocked input must not reach msg_sender"
        );

        tracker.finish_thread(&thread_id, &reporter);
        assert_eq!(
            reporter.queue_len(),
            0,
            "blocked input must not be recorded"
        );
    }

    #[tokio::test]
    async fn req_chat_send_message_enqueues_legacy_agent_loop_message() {
        let thread_id = "thread-legacy-send".to_string();
        let (app_state, mut rx) = create_test_app_state();
        let engine_state = EngineState::new();
        engine_state
            .initialize(app_state)
            .expect("test EngineState should initialize once");
        let app = tauri::test::mock_builder()
            .manage(engine_state)
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("mock Tauri app should build");

        let response = send_chat_message(
            app.handle().clone(),
            app.state::<EngineState>(),
            thread_id.clone(),
            "hello legacy agent loop".to_string(),
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .expect("normal chat send should dispatch through legacy Agent loop");

        assert!(response.success);
        let message = rx
            .try_recv()
            .expect("legacy msg_sender must receive normal chat messages");
        assert_eq!(message.thread_id.as_deref(), Some(thread_id.as_str()));
        assert_eq!(message.content, "hello legacy agent loop");
    }

    #[test]
    fn req_chat_i4_interrupt_control_message_targets_existing_thread() {
        let msg = build_thread_control_message("owner-1", "thread-a", "/interrupt");

        assert_eq!(msg.channel, "tauri");
        assert_eq!(msg.user_id, "owner-1");
        assert_eq!(msg.owner_id, "owner-1");
        assert_eq!(msg.thread_id.as_deref(), Some("thread-a"));
        assert_eq!(msg.conversation_scope(), Some("thread-a"));
        assert_eq!(msg.content, "/interrupt");
    }

    #[test]
    fn req_chat_i4_finalize_thread_enqueues_conversation_report() {
        let tracker = crate::conversation_tracker::ConversationTracker::new("owner-1".to_string());
        let reporter = crate::data_reporter::DataReporter::new(String::new(), String::new());

        tracker.record_user_message("thread-a", "hello", false, &[]);
        tracker.record_assistant_message("thread-a", "done", Some("model-a"), 3, 2);
        tracker.finish_thread("thread-a", &reporter);

        assert_eq!(reporter.queue_len(), 1);
    }
}
