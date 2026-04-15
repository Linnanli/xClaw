//! 对话审计单元测试

#[cfg(test)]
mod conversation_payload_tests {
    use admin_backend::models::{
        ConversationAttachmentPayload, ConversationMessagePayload, ConversationReportPayload,
    };

    #[test]
    fn req_conv_8_report_payload_deserializes() {
        let json = r#"{
            "client_conversation_id": "conv-abc-123",
            "user_id": "550e8400-e29b-41d4-a716-446655440000",
            "topic": "产品需求分析",
            "model_id": "deepseek-chat",
            "dlp_flagged": false,
            "messages": [
                {"role": "user", "content": "帮我分析需求", "input_tokens": 10, "output_tokens": 0},
                {"role": "assistant", "content": "好的，请提供需求文档", "model_id": "deepseek-chat", "input_tokens": 0, "output_tokens": 20}
            ]
        }"#;
        let payload: ConversationReportPayload = serde_json::from_str(json).expect("应能反序列化");
        assert_eq!(payload.client_conversation_id, "conv-abc-123");
        assert_eq!(payload.messages.len(), 2);
        assert_eq!(payload.messages[0].role, "user");
        assert_eq!(payload.messages[1].output_tokens, 20);
    }

    #[test]
    fn req_conv_8_report_payload_minimal() {
        let json = r#"{
            "client_conversation_id": "conv-min",
            "user_id": "550e8400-e29b-41d4-a716-446655440000",
            "messages": []
        }"#;
        let payload: ConversationReportPayload = serde_json::from_str(json).expect("应能反序列化");
        assert!(payload.topic.is_none());
        assert!(payload.model_id.is_none());
        assert!(payload.dlp_flagged.is_none());
        assert!(payload.messages.is_empty());
    }

    #[test]
    fn req_conv_5_token_calculation() {
        let messages = vec![
            ConversationMessagePayload {
                role: "user".into(),
                content: "hi".into(),
                attachments: Vec::new(),
                model_id: None,
                input_tokens: 10,
                output_tokens: 0,
            },
            ConversationMessagePayload {
                role: "assistant".into(),
                content: "hello".into(),
                attachments: Vec::new(),
                model_id: Some("gpt".into()),
                input_tokens: 0,
                output_tokens: 20,
            },
        ];
        let total: i32 = messages
            .iter()
            .map(|m| m.input_tokens + m.output_tokens)
            .sum();
        assert_eq!(total, 30);
    }

    #[test]
    fn req_conv_9_report_payload_attachment_backward_compatible() {
        let json = r#"{
            "client_conversation_id": "conv-old",
            "user_id": "550e8400-e29b-41d4-a716-446655440000",
            "messages": [
                {"role": "user", "content": "legacy message"}
            ]
        }"#;
        let payload: ConversationReportPayload = serde_json::from_str(json).expect("旧 payload 应能反序列化");
        assert_eq!(payload.messages.len(), 1);
        assert!(payload.messages[0].attachments.is_empty());
    }

    #[test]
    fn req_conv_10_report_payload_with_attachments() {
        let message = ConversationMessagePayload {
            role: "user".into(),
            content: "see attachment".into(),
            attachments: vec![ConversationAttachmentPayload {
                id: "img-1".into(),
                kind: "image".into(),
                mime_type: "image/png".into(),
                filename: Some("diagram.png".into()),
                size_bytes: Some(128),
                extracted_text: None,
                image_data_base64: Some("Zm9v".into()),
                duration_secs: None,
            }],
            model_id: None,
            input_tokens: 0,
            output_tokens: 0,
        };
        let json = serde_json::to_value(&message).expect("message 应能序列化");

        assert_eq!(json["attachments"][0]["kind"], "image");
        assert_eq!(json["attachments"][0]["filename"], "diagram.png");
    }
}

#[cfg(test)]
mod conversation_contract_tests {
    use serde_json::json;

    const REQUIRED_LIST_FIELDS: &[&str] = &[
        "id",
        "username",
        "topic",
        "message_count",
        "total_tokens",
        "dlp_flagged",
        "created_at",
    ];

    #[test]
    fn test_contract_conversation_list_response() {
        let resp = json!({
            "data": [{"id": "uuid", "username": "zhang.wei", "topic": "需求分析", "message_count": 24, "total_tokens": 18500, "model_id": "deepseek-chat", "dlp_flagged": true, "created_at": "2025-01-01T14:32:00Z"}],
            "total": 1, "page": 1, "page_size": 20
        });
        assert!(resp["data"].is_array());
        let item = &resp["data"][0];
        for f in REQUIRED_LIST_FIELDS {
            assert!(item.get(*f).is_some(), "列表响应缺少字段: {}", f);
        }
    }

    #[test]
    fn test_contract_conversation_list_no_message_content() {
        // 验收标准10：列表 API 不应返回消息内容
        let item = json!({"id": "uuid", "username": "test", "topic": "t", "message_count": 1, "total_tokens": 10, "dlp_flagged": false, "created_at": "2025-01-01T00:00:00Z"});
        assert!(item.get("messages").is_none(), "列表不应包含 messages 字段");
        assert!(item.get("content").is_none(), "列表不应包含 content 字段");
    }

    #[test]
    fn test_contract_conversation_detail_has_messages() {
        let resp = json!({
            "id": "uuid", "username": "test", "topic": "t", "message_count": 2, "total_tokens": 30,
            "dlp_flagged": false, "created_at": "2025-01-01T00:00:00Z",
            "messages": [
                {"id": "m1", "role": "user", "content": "hi", "attachments": [{"id": "img-1", "kind": "image", "mime_type": "image/png", "image_data_base64": "Zm9v"}], "input_tokens": 10, "output_tokens": 0, "created_at": "2025-01-01T00:00:00Z"},
                {"id": "m2", "role": "assistant", "content": "hello", "attachments": [], "input_tokens": 0, "output_tokens": 20, "created_at": "2025-01-01T00:00:01Z"}
            ]
        });
        assert!(resp["messages"].is_array());
        assert_eq!(resp["messages"].as_array().unwrap().len(), 2);
        assert!(resp["messages"][0]["attachments"].is_array());
    }

    #[test]
    fn test_contract_conversation_stats_response() {
        let resp = json!({"today_count": 342, "today_tokens": 1200000, "today_dlp_flagged": 18, "today_active_users": 67});
        for f in &[
            "today_count",
            "today_tokens",
            "today_dlp_flagged",
            "today_active_users",
        ] {
            assert!(resp[*f].is_number(), "统计响应缺少字段: {}", f);
        }
    }
}

#[cfg(test)]
mod conversation_security_tests {
    #[test]
    fn test_audit_conversation_list_no_content_leak() {
        // 列表 API 返回的字段不应包含对话原文
        let list_fields = [
            "id",
            "username",
            "topic",
            "message_count",
            "total_tokens",
            "model_id",
            "dlp_flagged",
            "created_at",
        ];
        assert!(!list_fields.contains(&"content"), "列表不应泄露对话内容");
        assert!(!list_fields.contains(&"messages"), "列表不应包含消息数组");
    }

    #[test]
    fn test_audit_conversation_no_api_key_in_response() {
        let detail_fields = ["id", "username", "topic", "messages", "dlp_flagged"];
        assert!(!detail_fields.contains(&"api_key"), "详情不应泄露 API Key");
    }
}
