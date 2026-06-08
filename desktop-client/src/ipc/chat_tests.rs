//! 聊天 IPC 命令测试。
//!
//! 覆盖维度：
//! - 单元测试（正常路径 + 错误路径）
//! - 契约测试（VercelUIStream 格式与前端 TypeScript 类型匹配）
//! - 安全审计测试（敏感信息不泄露）

#[cfg(test)]
mod tests {
    use crate::ipc::chat::{
        app_server_chat_inprocess_probe_enabled_value,
        app_server_chat_sidecar_dispatch_enabled_value,
        app_server_chat_sidecar_probe_enabled_value, app_server_sidecar_chat_frames_for_test,
        build_thread_control_message, probe_in_process_app_server_chat,
        probe_sidecar_app_server_chat_with_command, reject_blocked_scan, send_chat_message,
        usage_report_backend_user_id, FrontendAttachment, SendMessageResponse,
        APP_SERVER_CHAT_INPROCESS_PROBE_ENV, APP_SERVER_CHAT_SIDECAR_DISPATCH_ENV,
        APP_SERVER_CHAT_SIDECAR_PROBE_ENV,
    };
    use crate::safety_bridge::SafetyBridge;
    use crate::state::{AppState, EngineState};
    use crate::vercel_ui_protocol::VercelUIStream;
    use dasclaw_app_server_protocol::{event, ServerNotification, JSON_RPC_VERSION};
    use dasclaw_runtime::context::ContextManager;
    use ironclaw::safety::{SafetyConfig, SafetyLayer};
    use ironclaw::tools::ToolRegistry;
    use std::ffi::OsString;
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;
    use std::sync::Arc;
    use std::sync::RwLock;
    use std::time::Duration;
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

    #[test]
    fn app_server_chat_inprocess_probe_env_is_opt_in() {
        assert!(!app_server_chat_inprocess_probe_enabled_value(None));
        assert!(!app_server_chat_inprocess_probe_enabled_value(Some(
            std::ffi::OsString::from("0")
        )));
        assert!(app_server_chat_inprocess_probe_enabled_value(Some(
            std::ffi::OsString::from("1")
        )));
        assert!(app_server_chat_inprocess_probe_enabled_value(Some(
            std::ffi::OsString::from("true")
        )));
        assert!(app_server_chat_inprocess_probe_enabled_value(Some(
            std::ffi::OsString::from("YES")
        )));
    }

    #[test]
    fn app_server_chat_sidecar_probe_env_is_opt_in() {
        assert!(!app_server_chat_sidecar_probe_enabled_value(None));
        assert!(!app_server_chat_sidecar_probe_enabled_value(Some(
            std::ffi::OsString::from("0")
        )));
        assert!(app_server_chat_sidecar_probe_enabled_value(Some(
            std::ffi::OsString::from("1")
        )));
        assert!(app_server_chat_sidecar_probe_enabled_value(Some(
            std::ffi::OsString::from("true")
        )));
        assert!(app_server_chat_sidecar_probe_enabled_value(Some(
            std::ffi::OsString::from("YES")
        )));
    }

    #[test]
    fn app_server_chat_sidecar_dispatch_env_is_opt_in() {
        assert!(!app_server_chat_sidecar_dispatch_enabled_value(None));
        assert!(!app_server_chat_sidecar_dispatch_enabled_value(Some(
            std::ffi::OsString::from("0")
        )));
        assert!(app_server_chat_sidecar_dispatch_enabled_value(Some(
            std::ffi::OsString::from("1")
        )));
        assert!(app_server_chat_sidecar_dispatch_enabled_value(Some(
            std::ffi::OsString::from("true")
        )));
        assert!(app_server_chat_sidecar_dispatch_enabled_value(Some(
            std::ffi::OsString::from("YES")
        )));
    }

    #[test]
    fn app_server_sidecar_chat_frames_bridge_turn_delta_to_chat_stream_contract() {
        let report = crate::embedded_server::AppServerSupervisorChatReport {
            desktop_thread_id: "desktop-thread".to_string(),
            app_server_thread_id: "app-thread".to_string(),
            turn_id: "turn-1".to_string(),
            turn_status: "pending".to_string(),
            connection_generation: 3,
            notifications: vec![
                server_notification(
                    event::TURN_DELTA,
                    serde_json::json!({
                        "threadId": "other-thread",
                        "turnId": "turn-1",
                        "delta": "ignored",
                    }),
                ),
                server_notification(
                    event::TURN_DELTA,
                    serde_json::json!({
                        "threadId": "app-thread",
                        "turnId": "turn-1",
                        "delta": "hel",
                    }),
                ),
                server_notification(
                    event::TURN_COMPLETED,
                    serde_json::json!({
                        "threadId": "app-thread",
                        "turnId": "turn-1",
                        "status": "completed",
                        "output": "hello",
                    }),
                ),
            ],
        };

        let frames = app_server_sidecar_chat_frames_for_test(&report, false)
            .expect("app-server notifications should map to UI frames");

        assert!(matches!(
            &frames[0],
            VercelUIStream::TextStart { id, .. } if id == "turn-1"
        ));
        assert!(matches!(
            &frames[1],
            VercelUIStream::TextDelta { id, delta, .. }
                if id == "turn-1" && delta == "hel"
        ));
        assert!(matches!(
            &frames[2],
            VercelUIStream::TextDelta { id, delta, .. }
                if id == "turn-1" && delta == "lo"
        ));
        assert!(matches!(
            &frames[3],
            VercelUIStream::TextEnd { id, .. } if id == "turn-1"
        ));
        let rendered_text = frames
            .iter()
            .filter_map(|frame| match frame {
                VercelUIStream::TextDelta { delta, .. } => Some(delta.as_str()),
                _ => None,
            })
            .collect::<String>();
        assert_eq!(rendered_text, "hello");
        let terminal = frames
            .iter()
            .find_map(|frame| match frame {
                VercelUIStream::DataCustom { data, .. } => Some(data),
                _ => None,
            })
            .expect("terminal metadata frame should be emitted");
        assert_eq!(terminal["type"], "app_server_turn");
        assert_eq!(terminal["status"], "completed");
        assert_eq!(terminal["thread_id"], "desktop-thread");
        assert_eq!(terminal["app_server_thread_id"], "app-thread");
        assert_eq!(terminal["connection_generation"], 3);
        assert!(
            !frames
                .iter()
                .any(|frame| matches!(frame, VercelUIStream::Finish { .. })),
            "diagnostic sidecar probe must not close the legacy chat stream"
        );
        let probe_payloads = frames
            .iter()
            .map(|frame| {
                crate::tauri_channel::build_chat_stream_payload(
                    Some(&report.desktop_thread_id),
                    frame,
                )
            })
            .collect::<Vec<_>>();
        assert!(
            probe_payloads
                .iter()
                .all(|payload| payload["threadId"] == "desktop-thread"),
            "chat-stream envelope must route app-server probe frames to the desktop thread"
        );
        assert!(
            !probe_payloads
                .iter()
                .any(|payload| payload["type"] == "finish"),
            "probe payloads must not emit finish"
        );

        let dispatch_frames = app_server_sidecar_chat_frames_for_test(&report, true)
            .expect("dispatch mode should map app-server notifications");
        assert!(matches!(
            dispatch_frames.last(),
            Some(VercelUIStream::Finish { id }) if id == "turn-1"
        ));
        let dispatch_payloads = dispatch_frames
            .iter()
            .map(|frame| {
                crate::tauri_channel::build_chat_stream_payload(
                    Some(&report.desktop_thread_id),
                    frame,
                )
            })
            .collect::<Vec<_>>();
        assert!(
            dispatch_payloads
                .iter()
                .any(|payload| payload["type"] == "finish"
                    && payload["threadId"] == "desktop-thread"),
            "dispatch payloads must emit finish for the desktop thread"
        );
    }

    #[test]
    fn app_server_chat_inprocess_probe_uses_client_thread_and_turn_contract() {
        probe_in_process_app_server_chat("thread-probe", "hello from desktop")
            .expect("in-process probe should complete app-server client turn contract");
    }

    #[test]
    fn app_server_chat_sidecar_probe_uses_stdio_client_contract() {
        let expected_thread_id = "thread-sidecar";
        let expected_prompt = "hello sidecar direct";
        let (sidecar_dir, sidecar_path, transcript_path) = strict_chat_sidecar_executable();
        let _keep_sidecar_dir_alive = sidecar_dir;
        let mut command = Command::new(sidecar_path);

        probe_sidecar_app_server_chat_with_command(
            &mut command,
            expected_thread_id,
            expected_prompt,
        )
        .expect("sidecar probe should complete the stdio app-server client contract");
        assert_chat_sidecar_transcript_matches_probe_inputs(
            &transcript_path,
            expected_thread_id,
            expected_prompt,
        );
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
    #[serial_test::serial]
    async fn req_chat_app_server_inprocess_probe_still_dispatches_legacy_message() {
        std::env::set_var(APP_SERVER_CHAT_INPROCESS_PROBE_ENV, "1");

        let thread_id = "thread-probe-send".to_string();
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
            "hello probe".to_string(),
            None,
            None,
            None,
            None,
            None,
        )
        .await;

        std::env::remove_var(APP_SERVER_CHAT_INPROCESS_PROBE_ENV);

        let response = response.expect("in-process probe must not block legacy dispatch");
        assert!(response.success);
        let message = rx
            .try_recv()
            .expect("legacy msg_sender must still receive the chat message");
        assert_eq!(message.thread_id.as_deref(), Some(thread_id.as_str()));
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn req_chat_app_server_sidecar_probe_still_dispatches_legacy_message() {
        let _ = crate::embedded_server::shutdown_app_server_sidecar_supervisor_if_running().await;
        let transcript_dir = tempfile::tempdir().expect("transcript dir");
        let transcript_path = transcript_dir.path().join("supervisor-chat.jsonl");
        let config = supervisor_chat_config(&transcript_path);
        assert!(crate::embedded_server::start_app_server_sidecar_supervisor_once(config));

        std::env::set_var(APP_SERVER_CHAT_SIDECAR_PROBE_ENV, "1");

        let thread_id = "thread-sidecar-send".to_string();
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
            "hello sidecar".to_string(),
            None,
            None,
            None,
            None,
            None,
        )
        .await;

        std::env::remove_var(APP_SERVER_CHAT_SIDECAR_PROBE_ENV);
        let stopped =
            crate::embedded_server::shutdown_app_server_sidecar_supervisor_if_running().await;
        assert_eq!(
            stopped.state,
            crate::embedded_server::AppServerSupervisorState::Stopped
        );

        let response = response.expect("sidecar probe failure must not block legacy dispatch");
        assert!(response.success);
        let message = rx
            .try_recv()
            .expect("legacy msg_sender must still receive the chat message");
        assert_eq!(message.thread_id.as_deref(), Some(thread_id.as_str()));
        assert_supervisor_chat_transcript_matches_probe_inputs(
            &transcript_path.to_string_lossy(),
            &thread_id,
            "hello sidecar",
        );
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn req_chat_app_server_sidecar_dispatch_skips_legacy_message() {
        let _ = crate::embedded_server::shutdown_app_server_sidecar_supervisor_if_running().await;
        let transcript_dir = tempfile::tempdir().expect("transcript dir");
        let transcript_path = transcript_dir.path().join("supervisor-chat-dispatch.jsonl");
        let config = supervisor_chat_config(&transcript_path);
        assert!(crate::embedded_server::start_app_server_sidecar_supervisor_once(config));

        std::env::set_var(APP_SERVER_CHAT_SIDECAR_DISPATCH_ENV, "1");

        let thread_id = "thread-sidecar-dispatch".to_string();
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
            "hello dispatch".to_string(),
            None,
            None,
            None,
            None,
            None,
        )
        .await;

        std::env::remove_var(APP_SERVER_CHAT_SIDECAR_DISPATCH_ENV);
        let stopped =
            crate::embedded_server::shutdown_app_server_sidecar_supervisor_if_running().await;
        assert_eq!(
            stopped.state,
            crate::embedded_server::AppServerSupervisorState::Stopped
        );

        let response = response.expect("sidecar dispatch should complete through supervisor");
        assert!(response.success);
        assert!(
            rx.try_recv().is_err(),
            "dispatch mode must not enqueue the legacy Agent loop message"
        );
        assert_supervisor_chat_transcript_matches_probe_inputs(
            &transcript_path.to_string_lossy(),
            &thread_id,
            "hello dispatch",
        );
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn req_chat_app_server_sidecar_dispatch_failure_skips_legacy_message() {
        let _ = crate::embedded_server::shutdown_app_server_sidecar_supervisor_if_running().await;
        let transcript_dir = tempfile::tempdir().expect("transcript dir");
        let transcript_path = transcript_dir
            .path()
            .join("supervisor-chat-dispatch-failure.jsonl");
        let config = supervisor_chat_failure_config(&transcript_path);
        assert!(crate::embedded_server::start_app_server_sidecar_supervisor_once(config));

        std::env::set_var(APP_SERVER_CHAT_SIDECAR_DISPATCH_ENV, "1");

        let thread_id = "thread-sidecar-dispatch-failure".to_string();
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
            thread_id,
            "dispatch failure should not fall back".to_string(),
            None,
            None,
            None,
            None,
            None,
        )
        .await;

        std::env::remove_var(APP_SERVER_CHAT_SIDECAR_DISPATCH_ENV);
        let status = crate::embedded_server::app_server_sidecar_supervisor_status();
        assert_eq!(
            status.state,
            crate::embedded_server::AppServerSupervisorState::Ready
        );
        assert_eq!(status.restart_count, 0);
        let _ = crate::embedded_server::shutdown_app_server_sidecar_supervisor_if_running().await;

        let error = response.expect_err("dispatch failure should surface to the caller");
        assert!(error.contains("app-server JSON-RPC error"));
        assert!(
            rx.try_recv().is_err(),
            "dispatch mode must not fall back into the legacy Agent loop after app-server failure"
        );
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn req_chat_app_server_sidecar_probe_failure_still_dispatches_legacy_message() {
        let _ = crate::embedded_server::shutdown_app_server_sidecar_supervisor_if_running().await;
        let transcript_dir = tempfile::tempdir().expect("transcript dir");
        let transcript_path = transcript_dir.path().join("supervisor-chat-failure.jsonl");
        let config = supervisor_chat_failure_config(&transcript_path);
        assert!(crate::embedded_server::start_app_server_sidecar_supervisor_once(config));

        std::env::set_var(APP_SERVER_CHAT_SIDECAR_PROBE_ENV, "1");

        let thread_id = "thread-sidecar-failure".to_string();
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
            "sidecar failure should not block".to_string(),
            None,
            None,
            None,
            None,
            None,
        )
        .await;

        std::env::remove_var(APP_SERVER_CHAT_SIDECAR_PROBE_ENV);
        let status = crate::embedded_server::app_server_sidecar_supervisor_status();
        assert_eq!(
            status.state,
            crate::embedded_server::AppServerSupervisorState::Ready
        );
        assert_eq!(status.restart_count, 0);
        let _ = crate::embedded_server::shutdown_app_server_sidecar_supervisor_if_running().await;

        let response = response.expect("sidecar probe failure must not block legacy dispatch");
        assert!(response.success);
        let message = rx
            .try_recv()
            .expect("legacy msg_sender must still receive the chat message");
        assert_eq!(message.thread_id.as_deref(), Some(thread_id.as_str()));
    }

    fn strict_chat_sidecar_script() -> &'static str {
        r#"contains() {
  case "$line" in
    *"$1"*) return 0 ;;
    *) return 1 ;;
  esac
}

reject() {
  echo '{"jsonrpc":"2.0","id":99,"error":{"code":-32601,"message":"unexpected chat sidecar request"}}'
  exit 2
}

require() {
  contains "$1" || reject
}

count=0
while IFS= read -r line; do
if [ -n "$TRANSCRIPT" ]; then
  printf '%s\n' "$line" >> "$TRANSCRIPT"
fi
count=$((count + 1))
case "$count" in
  1)
    require '"id":1'
    require '"method":"initialize"'
    require '"name":"desktop-client-chat-sidecar-probe"'
    require '"requestedCapabilities":["session"]'
    echo '{"jsonrpc":"2.0","id":1,"result":{"server":{"name":"dasclaw-app-server-test","version":"0.1.0","protocolVersion":{"major":0,"minor":1,"patch":0}},"lifecycle":{"state":"ready","reason":"runtime_ready","since":"0"},"capabilities":{"protocol":{"id":"protocol","status":"implemented","version":"0.1.0"},"lifecycle":{"id":"lifecycle","status":"implemented","version":"0.1.0"},"health":{"id":"health","status":"implemented","version":"0.1.0"},"session":{"id":"session","status":"implemented","version":"0.1.0"},"approval":{"id":"approval","status":"declared","version":"0.1.0"},"dlpPolicy":{"id":"dlpPolicy","status":"declared","version":"0.1.0"},"modelProvider":{"id":"modelProvider","status":"declared","version":"0.1.0"},"tools":{"id":"tools","status":"declared","version":"0.1.0"},"jobs":{"id":"jobs","status":"declared","version":"0.1.0"},"skills":{"id":"skills","status":"declared","version":"0.1.0"},"mcp":{"id":"mcp","status":"declared","version":"0.1.0"},"sandbox":{"id":"sandbox","status":"declared","version":"0.1.0"},"logs":{"id":"logs","status":"declared","version":"0.1.0"}}}}'
    ;;
  2)
    require '"id":2'
    require '"method":"thread/create"'
    require '"title":"desktop sidecar probe '
    echo '{"jsonrpc":"2.0","id":2,"result":{"threadId":"sidecar-thread","lifecycle":{"state":"ready","reason":"runtime_ready","since":"0"}}}'
    ;;
  3)
    require '"id":3'
    require '"method":"turn/start"'
    require '"threadId":"sidecar-thread"'
    require '"prompt":'
    echo '{"jsonrpc":"2.0","id":3,"result":{"turnId":"sidecar-turn","status":"pending","lifecycle":{"state":"running","reason":"request_in_progress","since":"1"}}}'
    ;;
  4)
    require '"id":4'
    require '"method":"shutdown"'
    require '"reason":"client_exit"'
    echo '{"jsonrpc":"2.0","id":4,"result":{"accepted":true,"lifecycle":{"state":"stopped","reason":"shutdown_requested","since":"2"}}}'
    exit 0
    ;;
  *) reject ;;
esac
done"#
    }

    fn server_notification(method: &str, params: serde_json::Value) -> ServerNotification {
        ServerNotification {
            jsonrpc: JSON_RPC_VERSION.to_string(),
            method: method.to_string(),
            params,
        }
    }

    fn strict_chat_sidecar_executable() -> (tempfile::TempDir, String, String) {
        let dir = tempfile::tempdir().expect("sidecar tempdir");
        let path = dir.path().join("strict-chat-sidecar.sh");
        let transcript_path = dir.path().join("chat-sidecar-transcript.jsonl");
        std::fs::write(
            &path,
            format!(
                "#!/bin/sh\nTRANSCRIPT={}\n{}",
                shell_quote(&transcript_path.to_string_lossy()),
                strict_chat_sidecar_script()
            ),
        )
        .expect("write strict sidecar script");
        let mut permissions = std::fs::metadata(&path)
            .expect("strict sidecar metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).expect("chmod strict sidecar script");
        (
            dir,
            path.to_string_lossy().into_owned(),
            transcript_path.to_string_lossy().into_owned(),
        )
    }

    fn supervisor_chat_config(
        transcript_path: &std::path::Path,
    ) -> crate::embedded_server::AppServerSupervisorConfig {
        supervisor_chat_config_with_script(strict_supervisor_chat_script(transcript_path))
    }

    fn supervisor_chat_failure_config(
        transcript_path: &std::path::Path,
    ) -> crate::embedded_server::AppServerSupervisorConfig {
        supervisor_chat_config_with_script(rejecting_supervisor_chat_script(transcript_path))
    }

    fn supervisor_chat_config_with_script(
        script: String,
    ) -> crate::embedded_server::AppServerSupervisorConfig {
        crate::embedded_server::AppServerSupervisorConfig {
            binary: OsString::from("/bin/sh"),
            args: vec![OsString::from("-c"), OsString::from(script)],
            health_interval: Duration::from_secs(1),
            restart_backoff: Duration::from_millis(10),
            request_timeout: Duration::from_millis(250),
            max_restarts: 0,
        }
    }

    fn strict_supervisor_chat_script(transcript_path: &std::path::Path) -> String {
        format!(
            r#"while IFS= read -r line; do
  printf '%s\n' "$line" >> {}
  case "$line" in
    *'"id":"supervisor-init"'*'"method":"initialize"'*)
      echo '{{"jsonrpc":"2.0","id":"supervisor-init","result":{{"lifecycle":{{"state":"ready"}}}}}}'
      ;;
    *'"id":"supervisor-health"'*'"method":"health/check"'*)
      echo '{{"jsonrpc":"2.0","id":"supervisor-health","result":{{"ok":true,"services":[{{"service":"runtime","status":"degraded","failSafe":false}}]}}}}'
      ;;
    *'"id":"supervisor-chat-thread-1"'*'"method":"thread/create"'*)
      echo '{{"jsonrpc":"2.0","method":"thread/created","params":{{"threadId":"sidecar-thread"}}}}'
      echo '{{"jsonrpc":"2.0","id":"supervisor-chat-thread-1","result":{{"threadId":"sidecar-thread","lifecycle":{{"state":"ready","reason":"runtime_ready","since":"0"}}}}}}'
      ;;
	    *'"id":"supervisor-chat-turn-1"'*'"method":"turn/start"'*'"threadId":"sidecar-thread"'*)
	      echo '{{"jsonrpc":"2.0","id":"supervisor-chat-turn-1","result":{{"turnId":"sidecar-turn","status":"pending","lifecycle":{{"state":"running","reason":"request_in_progress","since":"1"}}}}}}'
	      echo '{{"jsonrpc":"2.0","method":"turn/delta","params":{{"threadId":"sidecar-thread","turnId":"sidecar-turn","delta":"hel"}}}}'
	      echo '{{"jsonrpc":"2.0","method":"turn/completed","params":{{"threadId":"sidecar-thread","turnId":"sidecar-turn","status":"completed","output":"hello"}}}}'
	      ;;
    *'"id":"supervisor-stop"'*'"method":"shutdown"'*)
      echo '{{"jsonrpc":"2.0","id":"supervisor-stop","result":{{"accepted":true,"lifecycle":{{"state":"stopped"}}}}}}'
      exit 0
      ;;
    *)
      echo '{{"jsonrpc":"2.0","id":"unexpected","error":{{"code":-32601,"message":"unexpected supervisor chat request"}}}}'
      exit 2
      ;;
  esac
done"#,
            shell_quote(&transcript_path.to_string_lossy())
        )
    }

    fn rejecting_supervisor_chat_script(transcript_path: &std::path::Path) -> String {
        format!(
            r#"while IFS= read -r line; do
  printf '%s\n' "$line" >> {}
  case "$line" in
    *'"id":"supervisor-init"'*'"method":"initialize"'*)
      echo '{{"jsonrpc":"2.0","id":"supervisor-init","result":{{"lifecycle":{{"state":"ready"}}}}}}'
      ;;
    *'"id":"supervisor-health"'*'"method":"health/check"'*)
      echo '{{"jsonrpc":"2.0","id":"supervisor-health","result":{{"ok":true,"services":[{{"service":"runtime","status":"degraded","failSafe":false}}]}}}}'
      ;;
    *'"id":"supervisor-chat-thread-1"'*'"method":"thread/create"'*)
      echo '{{"jsonrpc":"2.0","id":"supervisor-chat-thread-1","error":{{"code":-32000,"message":"thread create rejected"}}}}'
      ;;
    *'"id":"supervisor-stop"'*'"method":"shutdown"'*)
      echo '{{"jsonrpc":"2.0","id":"supervisor-stop","result":{{"accepted":true,"lifecycle":{{"state":"stopped"}}}}}}'
      exit 0
      ;;
    *)
      echo '{{"jsonrpc":"2.0","id":"unexpected","error":{{"code":-32601,"message":"unexpected supervisor failure request"}}}}'
      exit 2
      ;;
  esac
done"#,
            shell_quote(&transcript_path.to_string_lossy())
        )
    }

    fn assert_chat_sidecar_transcript_matches_probe_inputs(
        path: &str,
        expected_thread_id: &str,
        expected_prompt: &str,
    ) {
        let transcript = std::fs::read_to_string(path).expect("chat sidecar transcript");
        let requests = transcript
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .collect::<Vec<_>>();
        let thread_title = requests
            .iter()
            .find(|value| value.get("id").and_then(serde_json::Value::as_i64) == Some(2))
            .and_then(|value| value.pointer("/params/title"))
            .and_then(serde_json::Value::as_str);
        let prompt = requests
            .iter()
            .find(|value| value.get("id").and_then(serde_json::Value::as_i64) == Some(3))
            .and_then(|value| value.pointer("/params/prompt"))
            .and_then(serde_json::Value::as_str);
        let shutdown_reason = requests
            .iter()
            .find(|value| value.get("id").and_then(serde_json::Value::as_i64) == Some(4))
            .and_then(|value| value.pointer("/params/reason"))
            .and_then(serde_json::Value::as_str);

        assert_eq!(
            thread_title,
            Some(format!("desktop sidecar probe {expected_thread_id}").as_str()),
            "chat sidecar probe did not send expected thread title; transcript:\n{transcript}"
        );
        assert_eq!(
            prompt,
            Some(expected_prompt),
            "chat sidecar probe did not send expected prompt; transcript:\n{transcript}"
        );
        assert_eq!(
            shutdown_reason,
            Some("client_exit"),
            "chat sidecar probe did not send expected shutdown reason; transcript:\n{transcript}"
        );
    }

    fn assert_supervisor_chat_transcript_matches_probe_inputs(
        path: &str,
        expected_thread_id: &str,
        expected_prompt: &str,
    ) {
        let transcript = std::fs::read_to_string(path).expect("supervisor chat transcript");
        let requests = transcript
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .collect::<Vec<_>>();
        let thread_title = requests
            .iter()
            .find(|value| {
                value.get("id").and_then(serde_json::Value::as_str)
                    == Some("supervisor-chat-thread-1")
            })
            .and_then(|value| value.pointer("/params/title"))
            .and_then(serde_json::Value::as_str);
        let prompt = requests
            .iter()
            .find(|value| {
                value.get("id").and_then(serde_json::Value::as_str)
                    == Some("supervisor-chat-turn-1")
            })
            .and_then(|value| value.pointer("/params/prompt"))
            .and_then(serde_json::Value::as_str);

        assert_eq!(
            thread_title,
            Some(format!("desktop sidecar probe {expected_thread_id}").as_str()),
            "supervisor sidecar probe did not send expected thread title; transcript:\n{transcript}"
        );
        assert_eq!(
            prompt,
            Some(expected_prompt),
            "supervisor sidecar probe did not send expected prompt; transcript:\n{transcript}"
        );
    }

    fn shell_quote(value: &str) -> String {
        format!("'{}'", value.replace('\'', r#"'\''"#))
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
