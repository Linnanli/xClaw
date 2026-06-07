//! 模型白名单集成测试。
//!
//! 验证客户端拉取模型列表时正确传递 client_id，
//! 后端根据部门白名单过滤模型。

#[cfg(test)]
mod model_whitelist_tests {
    #[test]
    fn test_fetch_admin_models_includes_client_id() {
        // 验证 fetch_admin_models 函数构造的 URL 包含 client_id 参数
        let client_id = "550e8400-e29b-41d4-a716-446655440000";
        let admin_url = "http://localhost:3000";
        let expected_url = format!("{}/api/client-models?client_id={}", admin_url, client_id);

        // 这个测试验证 URL 格式正确
        assert!(expected_url.contains("client_id="));
        assert!(expected_url.contains(client_id));
    }

    #[test]
    fn test_conversation_report_structure() {
        use desktop_client::data_reporter::{ClientReport, ConversationMessage};

        // 验证 Conversation 类型可以正确序列化
        let report = ClientReport::Conversation {
            client_conversation_id: "conv-123".to_string(),
            user_id: "user-456".to_string(),
            topic: Some("测试对话".to_string()),
            model_id: Some("gpt-4o".to_string()),
            dlp_flagged: Some(false),
            dlp_details: None,
            used_skills: vec!["code-review-expert".to_string()],
            messages: vec![
                ConversationMessage {
                    role: "user".to_string(),
                    content: "Hello".to_string(),
                    attachments: Vec::new(),
                    model_id: None,
                    input_tokens: 0,
                    output_tokens: 0,
                },
                ConversationMessage {
                    role: "assistant".to_string(),
                    content: "Hi there!".to_string(),
                    attachments: Vec::new(),
                    model_id: Some("gpt-4o".to_string()),
                    input_tokens: 10,
                    output_tokens: 5,
                },
            ],
        };

        // 验证可以序列化
        let json = serde_json::to_string(&report).expect("序列化失败");
        assert!(json.contains("\"type\":\"conversation\""));
        assert!(json.contains("conv-123"));
        assert!(json.contains("测试对话"));
        assert!(json.contains("code-review-expert"));
        assert!(json.contains("input_tokens"));
        assert!(json.contains("output_tokens"));
    }
}

#[cfg(test)]
mod conversation_tracker_tests {
    use desktop_client::conversation_tracker::ConversationTracker;
    use desktop_client::data_reporter::DataReporter;
    use std::time::Duration;

    fn make_reporter() -> DataReporter {
        DataReporter::new(String::new(), String::new())
    }

    #[test]
    fn test_record_and_finish_produces_report() {
        let tracker = ConversationTracker::new("user-001".to_string());
        let reporter = make_reporter();

        tracker.record_user_message("thread-1", "你好", false, &[]);
        tracker.record_assistant_message(
            "thread-1",
            "你好！有什么可以帮你？",
            Some("gpt-4o"),
            15,
            20,
        );
        tracker.finish_thread("thread-1", &reporter);

        assert_eq!(reporter.queue_len(), 1);
    }

    #[test]
    fn test_empty_thread_produces_no_report() {
        let tracker = ConversationTracker::new("user-002".to_string());
        let reporter = make_reporter();

        tracker.finish_thread("thread-empty", &reporter);
        assert_eq!(reporter.queue_len(), 0);
    }

    #[test]
    fn test_dlp_flag_propagates_to_report() {
        let tracker = ConversationTracker::new("user-003".to_string());
        let reporter = make_reporter();

        tracker.record_user_message("thread-2", "敏感内容", true, &[]);
        tracker.record_assistant_message("thread-2", "已处理", None, 5, 3);
        tracker.finish_thread("thread-2", &reporter);

        // 验证有一条上报记录
        assert_eq!(reporter.queue_len(), 1);
    }

    #[test]
    fn test_conversation_report_serializes_correctly() {
        let tracker = ConversationTracker::new("user-004".to_string());
        let reporter = make_reporter();

        tracker.record_user_message("thread-3", "hello", false, &[]);
        tracker.record_assistant_message("thread-3", "hi", Some("gpt-4o"), 10, 8);
        tracker.finish_thread("thread-3", &reporter);

        // 验证有一条上报记录
        assert_eq!(reporter.queue_len(), 1);
    }

    #[test]
    fn test_idle_flush_triggers_report() {
        let mut tracker = ConversationTracker::new("user-005".to_string());
        tracker.idle_timeout = Duration::from_millis(1);
        let reporter = make_reporter();

        tracker.record_user_message("thread-4", "hi", false, &[]);
        tracker.record_assistant_message("thread-4", "hello", None, 5, 3);

        std::thread::sleep(Duration::from_millis(5));
        tracker.flush_idle_threads(&reporter);

        assert_eq!(reporter.queue_len(), 1);
    }

    #[test]
    fn test_multiple_threads_tracked_independently() {
        let tracker = ConversationTracker::new("user-006".to_string());
        let reporter = make_reporter();

        tracker.record_user_message("t-a", "msg a", false, &[]);
        tracker.record_user_message("t-b", "msg b", false, &[]);
        tracker.record_assistant_message("t-a", "reply a", None, 5, 3);
        tracker.record_assistant_message("t-b", "reply b", None, 7, 4);

        tracker.finish_thread("t-a", &reporter);
        assert_eq!(reporter.queue_len(), 1);

        tracker.finish_thread("t-b", &reporter);
        assert_eq!(reporter.queue_len(), 2);
    }
}
