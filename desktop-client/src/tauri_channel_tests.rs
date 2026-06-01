//! TauriChannel 扩展测试。
//!
//! 覆盖维度：
//! - 契约测试（VercelUIStream/DataCustom 变体与前端 TypeScript 类型匹配）
//! - 安全审计测试（敏感信息不泄露到前端事件）
//! - 数据级覆盖（边界值、空值、特殊字符）

#[cfg(test)]
mod tests {
    use crate::vercel_ui_protocol::VercelUIStream;
    use ironclaw::channels::StatusUpdate;
    use serde_json::json;

    // =========================================================================
    // 契约测试 — VercelUIStream 事件与前端 TypeScript 类型匹配
    // =========================================================================

    /// 前端 VercelStreamEvent 类型定义（TauriRuntimeProvider.tsx）。
    /// DataCustom 事件的 data.type 字段标识逻辑事件类型。

    #[test]
    fn test_contract_response_event() {
        let event = VercelUIStream::DataCustom {
            id: None,
            data: json!({
                "type": "response",
                "message_id": "m-1",
                "content": "Hello",
                "thread_id": "t-1",
                "source": "chat",
            }),
        };
        let json: serde_json::Value = serde_json::to_value(&event).unwrap();

        assert_eq!(json["type"], "data-custom");
        assert_eq!(json["data"]["type"], "response");
        assert!(json["data"]["message_id"].is_string());
        assert!(json["data"]["content"].is_string());
        assert!(json["data"]["thread_id"].is_string());
    }

    #[test]
    fn test_contract_thinking_event() {
        // AI SDK v5 拒收空 id 的 reasoning-delta；lifecycle 必须分配有效 id。
        let event = VercelUIStream::ReasoningDelta {
            id: "r-42".into(),
            delta: "processing".into(),
            provider_metadata: None,
        };
        let json: serde_json::Value = serde_json::to_value(&event).unwrap();

        assert_eq!(json["type"], "reasoning-delta");
        assert_eq!(json["id"], "r-42");
        assert!(json["delta"].is_string());
    }

    #[test]
    fn test_contract_status_event() {
        let event = VercelUIStream::DataCustom {
            id: None,
            data: json!({
                "type": "status",
                "message": "ready",
                "level": "info",
            }),
        };
        let json: serde_json::Value = serde_json::to_value(&event).unwrap();

        assert_eq!(json["type"], "data-custom");
        assert_eq!(json["data"]["type"], "status");
        assert!(json["data"]["message"].is_string());
        assert!(json["data"]["level"].is_string());
    }

    #[test]
    fn test_contract_error_event() {
        let event = VercelUIStream::Error {
            error_text: "timeout".into(),
        };
        let json: serde_json::Value = serde_json::to_value(&event).unwrap();

        assert_eq!(json["type"], "error");
        assert_eq!(json["errorText"], "timeout");
    }

    #[test]
    fn test_contract_connection_status_event() {
        let event = VercelUIStream::DataCustom {
            id: None,
            data: json!({
                "type": "connection_status",
                "connected": true,
                "message": "ready",
            }),
        };
        let json: serde_json::Value = serde_json::to_value(&event).unwrap();

        assert_eq!(json["type"], "data-custom");
        assert_eq!(json["data"]["connected"], true);
    }

    // =========================================================================
    // 契约测试 — Vercel protocol 原生事件
    // =========================================================================

    #[test]
    fn test_contract_tool_input_start() {
        let event = VercelUIStream::ToolInputStart {
            tool_call_id: "tc-1".into(),
            tool_name: "shell".into(),
            provider_executed: Some(true),
            provider_metadata: None,
        };
        let json: serde_json::Value = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "tool-input-start");
        assert_eq!(json["toolCallId"], "tc-1");
        assert_eq!(json["toolName"], "shell");
    }

    #[test]
    fn test_contract_tool_input_available() {
        let event = VercelUIStream::ToolInputAvailable {
            tool_call_id: "tc-1".into(),
            tool_name: "shell".into(),
            input: json!({"command": "ls"}),
            provider_executed: Some(true),
            provider_metadata: None,
        };
        let json: serde_json::Value = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "tool-input-available");
        assert_eq!(json["input"]["command"], "ls");
    }

    #[test]
    fn test_contract_tool_output_available() {
        let event = VercelUIStream::ToolOutputAvailable {
            tool_call_id: "tc-1".into(),
            output: json!("file list"),
            provider_executed: Some(true),
        };
        let json: serde_json::Value = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "tool-output-available");
        assert_eq!(json["toolCallId"], "tc-1");
    }

    #[test]
    fn test_contract_tool_output_error() {
        let event = VercelUIStream::ToolOutputError {
            tool_call_id: "tc-1".into(),
            error_text: "permission denied".into(),
            provider_executed: Some(true),
        };
        let json: serde_json::Value = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "tool-output-error");
        assert_eq!(json["errorText"], "permission denied");
    }

    #[test]
    fn test_contract_text_delta() {
        let event = VercelUIStream::TextDelta {
            id: String::new(),
            delta: "partial".into(),
            provider_metadata: None,
        };
        let json: serde_json::Value = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "text-delta");
        assert_eq!(json["delta"], "partial");
    }

    #[test]
    fn test_contract_finish() {
        let event = VercelUIStream::Finish { id: "msg-1".into() };
        let json: serde_json::Value = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "finish");
        assert_eq!(json["id"], "msg-1");
    }

    #[test]
    fn test_contract_approval_needed_event() {
        let event = VercelUIStream::DataCustom {
            id: None,
            data: json!({
                "type": "approval_needed",
                "thread_id": "thread-1",
                "request_id": "r-1",
                "tool_name": "rm",
                "description": "delete file",
            }),
        };
        let json: serde_json::Value = serde_json::to_value(&event).unwrap();
        assert_eq!(json["data"]["type"], "approval_needed");
        assert!(json["data"]["request_id"].is_string());
        assert!(json["data"]["tool_name"].is_string());
    }

    #[test]
    fn test_contract_image_generated_event() {
        let event = VercelUIStream::DataCustom {
            id: None,
            data: json!({
                "type": "image_generated",
                "data_url": "data:image/png;base64,abc",
                "path": "/tmp/img.png",
            }),
        };
        let json: serde_json::Value = serde_json::to_value(&event).unwrap();
        assert_eq!(json["data"]["type"], "image_generated");
        assert!(json["data"]["data_url"].is_string());
    }

    #[test]
    fn test_contract_suggestions_event() {
        let event = VercelUIStream::DataCustom {
            id: None,
            data: json!({
                "type": "suggestions",
                "suggestions": ["try this", "or that"],
            }),
        };
        let json: serde_json::Value = serde_json::to_value(&event).unwrap();
        assert_eq!(json["data"]["type"], "suggestions");
        assert_eq!(json["data"]["suggestions"].as_array().unwrap().len(), 2);
    }

    // =========================================================================
    // 安全审计测试
    // =========================================================================

    /// 验证 ToolOutputError 不泄露 tool parameters。
    #[test]
    fn test_audit_tool_completed_no_parameters_leak() {
        let event = VercelUIStream::ToolOutputError {
            tool_call_id: "tc-1".into(),
            error_text: "connection refused".into(),
            provider_executed: Some(true),
        };
        let json_str = serde_json::to_string(&event).unwrap();

        assert!(
            !json_str.contains("password"),
            "password leaked to frontend"
        );
        assert!(
            !json_str.contains("parameters"),
            "parameters field leaked to frontend"
        );
    }

    /// 验证 ApprovalNeeded 双发帧（DataCustom + ToolInputAvailable）均不泄露 parameters 字段。
    #[test]
    fn test_audit_approval_needed_no_parameters_leak() {
        let status = StatusUpdate::ApprovalNeeded {
            request_id: "r-1".into(),
            tool_name: "write_file".into(),
            description: "Write to config file".into(),
            parameters: serde_json::json!({
                "path": "/etc/shadow",
                "content": "root:$6$secret_hash:19000:0:99999:7:::"
            }),
            allow_always: false,
        };
        let meta = json!({ "thread_id": "t-1" });
        let events = crate::tauri_channel::map_status_to_stream(&status, &meta);
        assert_eq!(
            events.len(),
            2,
            "ApprovalNeeded 应发 DataCustom + ToolInputAvailable 两帧（兼容期双发）"
        );
        // 全部帧都不能泄露 parameters
        for (idx, ev) in events.iter().enumerate() {
            let json_str = serde_json::to_string(ev).unwrap();
            assert!(
                !json_str.contains("/etc/shadow"),
                "event #{idx} sensitive path leaked"
            );
            assert!(
                !json_str.contains("secret_hash"),
                "event #{idx} sensitive content leaked"
            );
            assert!(
                !json_str.contains("parameters"),
                "event #{idx} parameters field leaked"
            );
        }
    }

    /// 验证 ApprovalNeeded 的第 2 帧是标准的 `tool-input-available`，
    /// 与前端 `approval_request` ToolUI 对齐。
    #[test]
    fn test_contract_approval_needed_emits_tool_input_available() {
        let status = StatusUpdate::ApprovalNeeded {
            request_id: "550e8400-e29b-41d4-a716-446655440000".into(),
            tool_name: "shell".into(),
            description: "rm -rf /tmp".into(),
            parameters: serde_json::json!({"cmd": "rm -rf /tmp"}),
            allow_always: false,
        };
        let events = crate::tauri_channel::map_status_to_stream(&status, &json!({}));
        assert_eq!(events.len(), 2);

        // 序列化后验证协议兼容 AI SDK v5 UIMessageChunk
        let tool_frame = serde_json::to_value(&events[1]).unwrap();
        assert_eq!(tool_frame["type"], "tool-input-available");
        assert_eq!(
            tool_frame["toolCallId"],
            "550e8400-e29b-41d4-a716-446655440000"
        );
        assert_eq!(tool_frame["toolName"], "approval_request");
        // input 是前端 ApprovalArgs 的子集（不含 parameters / allow_always）
        assert_eq!(
            tool_frame["input"]["request_id"],
            "550e8400-e29b-41d4-a716-446655440000"
        );
        assert_eq!(tool_frame["input"]["tool_name"], "shell");
        assert_eq!(tool_frame["input"]["description"], "rm -rf /tmp");
        assert!(tool_frame["input"].get("parameters").is_none());
    }

    /// 验证 AuthRequired 事件不泄露 auth_url 和 setup_url。
    #[test]
    fn test_audit_auth_required_no_url_leak() {
        let status = StatusUpdate::AuthRequired {
            extension_name: "github".into(),
            instructions: Some("Click the link".into()),
            auth_url: Some("https://github.com/login/oauth?client_secret=abc123".into()),
            setup_url: Some("https://internal.corp/setup?token=xyz".into()),
        };
        let events = crate::tauri_channel::map_status_to_stream(&status, &json!({}));
        assert_eq!(events.len(), 1);
        let json_str = serde_json::to_string(&events[0]).unwrap();
        assert!(!json_str.contains("client_secret"), "client_secret leaked");
        assert!(!json_str.contains("abc123"), "secret value leaked");
        assert!(!json_str.contains("internal.corp"), "internal URL leaked");
        assert!(!json_str.contains("token=xyz"), "token leaked");
    }

    /// 验证 Response DataCustom 事件只包含预期字段。
    #[test]
    fn test_audit_response_event_structure() {
        let event = VercelUIStream::DataCustom {
            id: None,
            data: json!({
                "type": "response",
                "message_id": "m-1",
                "content": "Here is the result",
                "thread_id": "t-1",
                "source": "chat",
            }),
        };
        let json: serde_json::Value = serde_json::to_value(&event).unwrap();

        // Verify DataCustom wrapper
        assert_eq!(json["type"], "data-custom");
        let data = json["data"].as_object().unwrap();
        let expected_fields: std::collections::HashSet<&str> =
            ["type", "message_id", "content", "thread_id", "source"]
                .iter()
                .copied()
                .collect();
        let actual_fields: std::collections::HashSet<&str> =
            data.keys().map(|k| k.as_str()).collect();
        assert_eq!(expected_fields, actual_fields);
    }

    // =========================================================================
    // 数据级覆盖 — 边界值和特殊字符
    // =========================================================================

    #[test]
    fn test_data_empty_content() {
        let event = VercelUIStream::DataCustom {
            id: None,
            data: json!({
                "type": "response",
                "message_id": "",
                "content": "",
                "thread_id": "",
                "source": "chat",
            }),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""content":"""#));
    }

    #[test]
    fn test_data_unicode_content() {
        let event = VercelUIStream::DataCustom {
            id: None,
            data: json!({
                "type": "response",
                "message_id": "m-1",
                "content": "你好世界 🌍 مر",
                "thread_id": "t-1",
                "source": "chat",
            }),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("你好世界"));
        assert!(json.contains("🌍"));
    }

    #[test]
    fn test_data_special_characters_in_content() {
        let content = r#"He said "hello" and <script>alert('xss')</script>"#;
        let event = VercelUIStream::DataCustom {
            id: None,
            data: json!({
                "type": "response",
                "message_id": "m-1",
                "content": content,
                "thread_id": "t-1",
                "source": "chat",
            }),
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed["data"]["content"]
            .as_str()
            .unwrap()
            .contains("<script>"));
    }

    #[test]
    fn test_data_very_long_content() {
        let long_content = "a".repeat(100_000);
        let event = VercelUIStream::DataCustom {
            id: None,
            data: json!({
                "type": "response",
                "message_id": "m-1",
                "content": long_content,
                "thread_id": "t-1",
                "source": "chat",
            }),
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["data"]["content"].as_str().unwrap().len(), 100_000);
    }

    #[test]
    fn test_data_suggestions_empty_list() {
        let event = VercelUIStream::DataCustom {
            id: None,
            data: json!({
                "type": "suggestions",
                "suggestions": [],
            }),
        };
        let json: serde_json::Value = serde_json::to_value(&event).unwrap();
        assert_eq!(json["data"]["suggestions"].as_array().unwrap().len(), 0);
    }

    // =========================================================================
    // StatusUpdate 映射完整性测试
    // =========================================================================

    /// 验证所有 StatusUpdate 变体映射后都产生有效的 VercelUIStream 事件。
    #[test]
    fn test_all_status_updates_produce_valid_stream_events() {
        let meta = json!({ "_tool_call_id": "tc-1", "_tool_arguments": {} });
        let empty = json!({});

        let cases: Vec<(StatusUpdate, &serde_json::Value)> = vec![
            // Thinking 不走纯映射；由 emit_status_stream 按 lifecycle 处理，见 reasoning_* 系列测试。
            (
                StatusUpdate::ToolStarted {
                    name: "test".into(),
                },
                &meta,
            ),
            (
                StatusUpdate::ToolCompleted {
                    name: "test".into(),
                    success: true,
                    error: None,
                    parameters: None,
                },
                &meta,
            ),
            (
                StatusUpdate::ToolCompleted {
                    name: "test".into(),
                    success: false,
                    error: Some("fail".into()),
                    parameters: None,
                },
                &meta,
            ),
            (
                StatusUpdate::ToolResult {
                    name: "test".into(),
                    preview: "test".into(),
                },
                &meta,
            ),
            // StreamChunk 不走纯映射；由 emit_status_stream 按 TextSessions lifecycle 处理，见 text_* 系列测试。
            (StatusUpdate::Status("test".into()), &empty),
            (
                StatusUpdate::JobStarted {
                    job_id: "j-1".into(),
                    title: "test".into(),
                    browse_url: "http://localhost".into(),
                },
                &empty,
            ),
            (
                StatusUpdate::ApprovalNeeded {
                    request_id: "r-1".into(),
                    tool_name: "test".into(),
                    description: "test".into(),
                    parameters: json!({}),
                    allow_always: true,
                },
                &empty,
            ),
            (
                StatusUpdate::AuthRequired {
                    extension_name: "test".into(),
                    instructions: None,
                    auth_url: None,
                    setup_url: None,
                },
                &empty,
            ),
            (
                StatusUpdate::AuthCompleted {
                    extension_name: "test".into(),
                    success: true,
                    message: "ok".into(),
                },
                &empty,
            ),
            (
                StatusUpdate::ImageGenerated {
                    data_url: "data:".into(),
                    path: None,
                },
                &empty,
            ),
            (
                StatusUpdate::Suggestions {
                    suggestions: vec![],
                },
                &empty,
            ),
        ];

        for (status, m) in &cases {
            let events = crate::tauri_channel::map_status_to_stream(status, m);
            assert!(
                !events.is_empty(),
                "status {:?} should produce events",
                status
            );
            for event in &events {
                serde_json::to_value(event).expect("should serialize");
            }
        }
    }
}
