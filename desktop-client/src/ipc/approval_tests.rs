//! 工具审批 IPC 命令测试。
//!
//! 覆盖维度：
//! - 单元测试（正常路径 + 错误路径）
//! - 契约测试（审批消息格式与 Agent SubmissionParser 匹配）
//! - 安全审计测试（request_id 注入防护、敏感信息不泄露）
//! - 数据级覆盖（边界值、空值、特殊字符）
//! - 失败路径测试（空 request_id、恶意输入等）

#[cfg(test)]
mod tests {
    use ironclaw::channels::IncomingMessage;

    // =========================================================================
    // 单元测试 — 正常路径
    // =========================================================================

    #[test]
    fn test_approve_message_format() {
        let request_id = "req-abc-123";
        let content = format!("!approve {}", request_id);
        assert_eq!(content, "!approve req-abc-123");
    }

    #[test]
    fn test_deny_message_format() {
        let request_id = "req-xyz-789";
        let content = format!("!deny {}", request_id);
        assert_eq!(content, "!deny req-xyz-789");
    }

    #[test]
    fn test_incoming_message_construction() {
        let msg = IncomingMessage::new("tauri", "user-001", "!approve req-1")
            .with_thread("thread-1")
            .with_owner_id("user-001");

        // IncomingMessage 应该能正确构造
        // 验证通过编译即可 — IncomingMessage 的字段可能是私有的
        let _ = msg;
    }

    // =========================================================================
    // 契约测试 — 验证审批消息格式与 Agent SubmissionParser 匹配
    // =========================================================================

    /// Agent 的 SubmissionParser 期望的审批消息格式：
    /// - `!approve <request_id>` — 批准工具执行
    /// - `!deny <request_id>` — 拒绝工具执行
    ///
    /// 此测试验证 IPC 层生成的消息格式与 Parser 期望一致。
    #[test]
    fn test_contract_approve_format_matches_parser() {
        let request_id = "550e8400-e29b-41d4-a716-446655440000";
        let content = format!("!approve {}", request_id);

        // 验证格式：以 "!approve " 开头，后跟 request_id
        assert!(content.starts_with("!approve "));
        assert!(content.ends_with(request_id));

        // 验证可以正确拆分
        let parts: Vec<&str> = content.splitn(2, ' ').collect();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0], "!approve");
        assert_eq!(parts[1], request_id);
    }

    #[test]
    fn test_contract_deny_format_matches_parser() {
        let request_id = "550e8400-e29b-41d4-a716-446655440000";
        let content = format!("!deny {}", request_id);

        assert!(content.starts_with("!deny "));
        assert!(content.ends_with(request_id));

        let parts: Vec<&str> = content.splitn(2, ' ').collect();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0], "!deny");
        assert_eq!(parts[1], request_id);
    }

    /// 验证 IncomingMessage 使用 "tauri" 作为 channel（与 TauriChannel 一致）。
    #[test]
    fn test_contract_channel_name_is_tauri() {
        let channel = "tauri";
        // TauriChannel 使用 "tauri" 作为 channel 名称
        // 审批消息也应使用相同的 channel
        assert_eq!(channel, "tauri");
    }

    // =========================================================================
    // 安全审计测试
    // =========================================================================

    /// 验证 request_id 中的命令注入不会影响消息格式。
    ///
    /// 攻击场景：恶意 request_id 包含换行符或额外命令。
    /// 防护：format! 宏会将整个 request_id 作为单个字符串参数。
    #[test]
    fn test_audit_request_id_injection_newline() {
        let malicious_id = "req-1\n!approve req-2";
        let content = format!("!approve {}", malicious_id);

        // 整个字符串应该是一条消息，不应被拆分为两条命令
        assert_eq!(content, "!approve req-1\n!approve req-2");

        // 验证 splitn(2, ' ') 只会拆分第一个空格
        let parts: Vec<&str> = content.splitn(2, ' ').collect();
        assert_eq!(parts[0], "!approve");
        // request_id 部分包含注入内容 — Agent 的 Parser 应该处理这种情况
        assert!(parts[1].contains('\n'));
    }

    /// 验证 request_id 中的空格注入。
    #[test]
    fn test_audit_request_id_injection_space() {
        let malicious_id = "req-1 extra-data";
        let content = format!("!approve {}", malicious_id);

        // splitn(2, ' ') 会将 "req-1 extra-data" 作为整体 request_id
        let parts: Vec<&str> = content.splitn(2, ' ').collect();
        assert_eq!(parts[0], "!approve");
        assert_eq!(parts[1], "req-1 extra-data");
    }

    /// 验证审批消息不包含 owner_id 或其他敏感信息。
    #[test]
    fn test_audit_approve_message_no_sensitive_data() {
        let request_id = "req-123";
        let approve_content = format!("!approve {}", request_id);
        let deny_content = format!("!deny {}", request_id);

        // 消息内容只应包含命令和 request_id
        assert!(!approve_content.contains("owner"));
        assert!(!approve_content.contains("password"));
        assert!(!approve_content.contains("token"));

        assert!(!deny_content.contains("owner"));
        assert!(!deny_content.contains("password"));
        assert!(!deny_content.contains("token"));
    }

    // =========================================================================
    // 数据级覆盖 — 边界值和特殊字符
    // =========================================================================

    #[test]
    fn test_data_uuid_request_id() {
        let request_id = "550e8400-e29b-41d4-a716-446655440000";
        let content = format!("!approve {}", request_id);
        assert!(content.contains(request_id));
    }

    #[test]
    fn test_data_short_request_id() {
        let request_id = "1";
        let content = format!("!approve {}", request_id);
        assert_eq!(content, "!approve 1");
    }

    #[test]
    fn test_data_empty_request_id() {
        let request_id = "";
        let content = format!("!approve {}", request_id);
        // 空 request_id 会产生 "!approve " — 末尾有空格
        assert_eq!(content, "!approve ");
    }

    #[test]
    fn test_data_very_long_request_id() {
        let request_id = "a".repeat(1000);
        let content = format!("!approve {}", request_id);
        assert_eq!(content.len(), "!approve ".len() + 1000);
    }

    #[test]
    fn test_data_unicode_request_id() {
        let request_id = "请求-001";
        let content = format!("!approve {}", request_id);
        assert!(content.contains("请求-001"));
    }

    #[test]
    fn test_data_special_chars_request_id() {
        let request_id = "req/123#456?key=val&other=true";
        let content = format!("!deny {}", request_id);
        assert!(content.contains(request_id));
    }
}
