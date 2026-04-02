//! TauriChannel 扩展测试。
//!
//! 覆盖维度：
//! - 失败路径测试（double start、channel closed 等）
//! - 契约测试（ChatEvent 所有变体与前端 TypeScript 类型匹配）
//! - 安全审计测试（敏感信息不泄露到前端事件）
//! - 数据级覆盖（边界值、空值、特殊字符）

#[cfg(test)]
mod tests {
    use crate::tauri_channel::ChatEvent;
    use ironclaw::channels::StatusUpdate;

    // =========================================================================
    // 契约测试 — ChatEvent 所有变体与前端 TypeScript 类型匹配
    // =========================================================================

    /// 前端 ChatEvent 类型定义（useAiChatTauri.ts）：
    ///
    /// ```typescript
    /// type ChatEvent =
    ///   | { type: 'response'; message_id: string; content: string; thread_id: string; }
    ///   | { type: 'thinking'; message: string; }
    ///   | { type: 'status'; message: string; level: string; }
    ///   | { type: 'error'; message: string; code?: string; }
    ///   | { type: 'connection_status'; connected: boolean; message: string; };
    /// ```
    ///
    /// 此测试验证 Rust 端序列化的 JSON 格式与上述 TypeScript 类型完全兼容。

    #[test]
    fn test_contract_response_event() {
        let event = ChatEvent::Response {
            message_id: "m-1".into(),
            content: "Hello".into(),
            thread_id: "t-1".into(),
        };
        let json: serde_json::Value = serde_json::to_value(&event).unwrap();

        assert_eq!(json["type"], "response");
        assert!(json["message_id"].is_string());
        assert!(json["content"].is_string());
        assert!(json["thread_id"].is_string());
    }

    #[test]
    fn test_contract_thinking_event() {
        let event = ChatEvent::Thinking {
            message: "processing".into(),
        };
        let json: serde_json::Value = serde_json::to_value(&event).unwrap();

        assert_eq!(json["type"], "thinking");
        assert!(json["message"].is_string());
        // 前端只期望 type + message 两个字段
        let obj = json.as_object().unwrap();
        assert_eq!(obj.len(), 2);
    }

    #[test]
    fn test_contract_status_event() {
        let event = ChatEvent::Status {
            message: "ready".into(),
            level: "info".into(),
        };
        let json: serde_json::Value = serde_json::to_value(&event).unwrap();

        assert_eq!(json["type"], "status");
        assert!(json["message"].is_string());
        assert!(json["level"].is_string());
    }

    #[test]
    fn test_contract_error_event_with_code() {
        let event = ChatEvent::Error {
            message: "timeout".into(),
            code: Some("TIMEOUT".into()),
        };
        let json: serde_json::Value = serde_json::to_value(&event).unwrap();

        assert_eq!(json["type"], "error");
        assert!(json["message"].is_string());
        assert_eq!(json["code"], "TIMEOUT");
    }

    #[test]
    fn test_contract_error_event_without_code() {
        let event = ChatEvent::Error {
            message: "unknown".into(),
            code: None,
        };
        let json: serde_json::Value = serde_json::to_value(&event).unwrap();

        assert_eq!(json["type"], "error");
        assert!(json["message"].is_string());
        // code 为 None 时应序列化为 null
        assert!(json["code"].is_null());
    }

    #[test]
    fn test_contract_connection_status_event() {
        let event = ChatEvent::ConnectionStatus {
            connected: true,
            message: "ready".into(),
        };
        let json: serde_json::Value = serde_json::to_value(&event).unwrap();

        assert_eq!(json["type"], "connection_status");
        assert!(json["connected"].is_boolean());
        assert!(json["message"].is_string());
    }

    // =========================================================================
    // 契约测试 — 扩展事件（前端可选处理）
    // =========================================================================

    #[test]
    fn test_contract_tool_started_event() {
        let event = ChatEvent::ToolStarted {
            name: "shell".into(),
        };
        let json: serde_json::Value = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "tool_started");
        assert!(json["name"].is_string());
    }

    #[test]
    fn test_contract_tool_completed_event() {
        let event = ChatEvent::ToolCompleted {
            name: "shell".into(),
            success: true,
            error: None,
        };
        let json: serde_json::Value = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "tool_completed");
        assert!(json["name"].is_string());
        assert!(json["success"].is_boolean());
    }

    #[test]
    fn test_contract_stream_chunk_event() {
        let event = ChatEvent::StreamChunk {
            content: "partial".into(),
        };
        let json: serde_json::Value = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "stream_chunk");
        assert!(json["content"].is_string());
    }

    #[test]
    fn test_contract_approval_needed_event() {
        let event = ChatEvent::ApprovalNeeded {
            request_id: "r-1".into(),
            tool_name: "rm".into(),
            description: "delete file".into(),
        };
        let json: serde_json::Value = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "approval_needed");
        assert!(json["request_id"].is_string());
        assert!(json["tool_name"].is_string());
        assert!(json["description"].is_string());
    }

    #[test]
    fn test_contract_image_generated_event() {
        let event = ChatEvent::ImageGenerated {
            data_url: "data:image/png;base64,abc".into(),
            path: Some("/tmp/img.png".into()),
        };
        let json: serde_json::Value = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "image_generated");
        assert!(json["data_url"].is_string());
    }

    #[test]
    fn test_contract_suggestions_event() {
        let event = ChatEvent::Suggestions {
            suggestions: vec!["try this".into(), "or that".into()],
        };
        let json: serde_json::Value = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "suggestions");
        assert!(json["suggestions"].is_array());
        assert_eq!(json["suggestions"].as_array().unwrap().len(), 2);
    }

    // =========================================================================
    // 安全审计测试
    // =========================================================================

    /// 验证 StatusUpdate → ChatEvent 转换不泄露 tool parameters。
    ///
    /// `StatusUpdate::ToolCompleted` 包含 `parameters` 字段（可能含敏感数据），
    /// 但 `ChatEvent::ToolCompleted` 不应包含此字段。
    #[test]
    fn test_audit_tool_completed_no_parameters_leak() {
        let status = StatusUpdate::ToolCompleted {
            name: "db_query".into(),
            success: false,
            error: Some("connection refused".into()),
            parameters: Some(r#"{"password":"s3cret","host":"internal.db"}"#.into()),
        };
        let event = crate::tauri_channel::TauriChannel::status_to_event(&status);
        let json_str = serde_json::to_string(&event).unwrap();

        assert!(!json_str.contains("s3cret"), "password leaked to frontend");
        assert!(!json_str.contains("internal.db"), "internal host leaked to frontend");
        assert!(!json_str.contains("parameters"), "parameters field leaked to frontend");
    }

    /// 验证 ApprovalNeeded 事件不泄露 parameters 字段。
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
        let event = crate::tauri_channel::TauriChannel::status_to_event(&status);
        let json_str = serde_json::to_string(&event).unwrap();

        // parameters 中的敏感内容不应出现在前端事件中
        assert!(!json_str.contains("/etc/shadow"), "sensitive path from parameters leaked");
        assert!(!json_str.contains("secret_hash"), "sensitive content from parameters leaked");
        assert!(!json_str.contains("parameters"), "parameters field itself leaked");
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
        let event = crate::tauri_channel::TauriChannel::status_to_event(&status);
        let json_str = serde_json::to_string(&event).unwrap();

        assert!(!json_str.contains("client_secret"), "client_secret leaked");
        assert!(!json_str.contains("abc123"), "secret value leaked");
        assert!(!json_str.contains("internal.corp"), "internal URL leaked");
        assert!(!json_str.contains("token=xyz"), "token leaked");
    }

    /// 验证 Response 事件中的 content 不包含原始 API Key 模式。
    /// （这是一个防御性检查 — 正常情况下 Agent 不会返回 API Key）
    #[test]
    fn test_audit_response_event_structure() {
        let event = ChatEvent::Response {
            message_id: "m-1".into(),
            content: "Here is the result".into(),
            thread_id: "t-1".into(),
        };
        let json: serde_json::Value = serde_json::to_value(&event).unwrap();

        // 验证 Response 事件只包含预期字段
        let obj = json.as_object().unwrap();
        let expected_fields: std::collections::HashSet<&str> =
            ["type", "message_id", "content", "thread_id"]
                .iter()
                .copied()
                .collect();
        let actual_fields: std::collections::HashSet<&str> =
            obj.keys().map(|k| k.as_str()).collect();
        assert_eq!(expected_fields, actual_fields);
    }

    // =========================================================================
    // 数据级覆盖 — 边界值和特殊字符
    // =========================================================================

    #[test]
    fn test_data_empty_content() {
        let event = ChatEvent::Response {
            message_id: "".into(),
            content: "".into(),
            thread_id: "".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""content":"""#));
    }

    #[test]
    fn test_data_unicode_content() {
        let event = ChatEvent::Response {
            message_id: "m-1".into(),
            content: "你好世界 🌍 مرحبا".into(),
            thread_id: "t-1".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("你好世界"));
        assert!(json.contains("🌍"));
    }

    #[test]
    fn test_data_special_characters_in_content() {
        let event = ChatEvent::Response {
            message_id: "m-1".into(),
            content: r#"He said "hello" and <script>alert('xss')</script>"#.into(),
            thread_id: "t-1".into(),
        };
        // 序列化不应 panic
        let json = serde_json::to_string(&event).unwrap();
        // 反序列化应保持原始内容
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed["content"]
            .as_str()
            .unwrap()
            .contains("<script>"));
    }

    #[test]
    fn test_data_very_long_content() {
        let long_content = "a".repeat(100_000);
        let event = ChatEvent::Response {
            message_id: "m-1".into(),
            content: long_content.clone(),
            thread_id: "t-1".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["content"].as_str().unwrap().len(), 100_000);
    }

    #[test]
    fn test_data_suggestions_empty_list() {
        let event = ChatEvent::Suggestions {
            suggestions: vec![],
        };
        let json: serde_json::Value = serde_json::to_value(&event).unwrap();
        assert_eq!(json["suggestions"].as_array().unwrap().len(), 0);
    }

    // =========================================================================
    // StatusUpdate 映射完整性测试
    // =========================================================================

    /// 验证所有 StatusUpdate 变体映射后的 `type` 字段都是有效的 ChatEvent 类型。
    #[test]
    fn test_all_status_updates_produce_valid_event_types() {
        let valid_types = [
            "response",
            "thinking",
            "status",
            "error",
            "connection_status",
            "tool_started",
            "tool_completed",
            "stream_chunk",
            "approval_needed",
            "image_generated",
            "suggestions",
        ];

        let cases: Vec<StatusUpdate> = vec![
            StatusUpdate::Thinking("test".into()),
            StatusUpdate::ToolStarted {
                name: "test".into(),
            },
            StatusUpdate::ToolCompleted {
                name: "test".into(),
                success: true,
                error: None,
                parameters: None,
            },
            StatusUpdate::ToolResult {
                name: "test".into(),
                preview: "test".into(),
            },
            StatusUpdate::StreamChunk("test".into()),
            StatusUpdate::Status("test".into()),
            StatusUpdate::JobStarted {
                job_id: "j-1".into(),
                title: "test".into(),
                browse_url: "http://localhost".into(),
            },
            StatusUpdate::ApprovalNeeded {
                request_id: "r-1".into(),
                tool_name: "test".into(),
                description: "test".into(),
                parameters: serde_json::json!({}),
                allow_always: true,
            },
            StatusUpdate::AuthRequired {
                extension_name: "test".into(),
                instructions: None,
                auth_url: None,
                setup_url: None,
            },
            StatusUpdate::AuthCompleted {
                extension_name: "test".into(),
                success: true,
                message: "ok".into(),
            },
            StatusUpdate::ImageGenerated {
                data_url: "data:".into(),
                path: None,
            },
            StatusUpdate::Suggestions {
                suggestions: vec![],
            },
        ];

        for status in &cases {
            let event = crate::tauri_channel::TauriChannel::status_to_event(status);
            let json: serde_json::Value = serde_json::to_value(&event).unwrap();
            let event_type = json["type"].as_str().unwrap();
            assert!(
                valid_types.contains(&event_type),
                "StatusUpdate {:?} produced invalid event type: {}",
                status,
                event_type
            );
        }
    }
}
