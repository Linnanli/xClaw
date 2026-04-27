//! 聊天 IPC 命令测试。
//!
//! 覆盖维度：
//! - 单元测试（正常路径 + 错误路径）
//! - 契约测试（VercelUIStream 格式与前端 TypeScript 类型匹配）
//! - 安全审计测试（敏感信息不泄露）

#[cfg(test)]
mod tests {
    use crate::ipc::chat::{usage_report_backend_user_id, FrontendAttachment, SendMessageResponse};
    use std::sync::RwLock;
    use uuid::Uuid;

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
}
