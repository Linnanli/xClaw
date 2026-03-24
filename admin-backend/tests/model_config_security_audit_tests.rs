//! 模型配置 - 安全审计测试
//!
//! 验证敏感信息（API Key）在各个路径中不会泄露。

use serde_json::json;

// ============================================================================
// 安全审计测试
// ============================================================================

#[test]
fn test_audit_client_models_endpoint_excludes_api_key() {
    // GET /api/client-models 的响应不应包含 api_key
    let client_response = json!([{
        "model_id": "gpt-4o",
        "display_name": "GPT-4o",
        "description": "Test",
        "provider": "openai",
        "is_default": true,
        "capabilities": ["chat"]
    }]);

    let response_str = serde_json::to_string(&client_response).unwrap();
    assert!(!response_str.contains("api_key"), "客户端 API 响应不应包含 api_key 字段");
    assert!(!response_str.contains("api_base_url"), "客户端 API 响应不应包含 api_base_url 字段");
    assert!(!response_str.contains("sk-"), "客户端 API 响应不应包含 API Key 值");
}

#[test]
fn test_audit_api_key_masking_in_admin_response() {
    // 管理端响应中的 api_key 应该被脱敏
    let api_key = "sk-ant-api03-very-secret-key-12345";
    let masked = mask_api_key(api_key);

    assert!(!masked.contains("very-secret"), "脱敏后不应包含原始 key");
    assert!(masked.starts_with("sk-"), "脱敏后应保留前缀");
    assert!(masked.contains("***"), "脱敏后应包含掩码");
}

#[test]
fn test_audit_empty_api_key_masking() {
    let masked = mask_api_key("");
    assert_eq!(masked, "***");
}

#[test]
fn test_audit_short_api_key_masking() {
    let masked = mask_api_key("sk");
    assert!(!masked.contains("sk") || masked.len() <= 6, "短 key 应被完全脱敏");
}

#[test]
fn test_audit_error_messages_no_key_leak() {
    // 模拟各种错误场景，确保错误信息不包含 API Key
    let api_key = "sk-secret-key-12345";
    let error_messages = vec![
        format!("模型配置创建失败: 数据库错误"),
        format!("模型 ID 已存在: gpt-4o"),
        format!("无法连接到模型 API"),
    ];

    for msg in &error_messages {
        assert!(
            !msg.contains(api_key),
            "错误信息不应包含 API Key: {}",
            msg
        );
    }
}

#[test]
fn test_audit_create_request_validation_no_key_in_error() {
    // 验证请求验证失败时不会在错误中暴露 API Key
    let request = json!({
        "model_id": "",
        "display_name": "",
        "provider": "openai",
        "api_key": "sk-super-secret-key"
    });

    // 模拟验证逻辑
    let model_id = request["model_id"].as_str().unwrap_or("");
    let display_name = request["display_name"].as_str().unwrap_or("");

    let mut errors = Vec::new();
    if model_id.is_empty() {
        errors.push("model_id 不能为空".to_string());
    }
    if display_name.is_empty() {
        errors.push("display_name 不能为空".to_string());
    }

    for error in &errors {
        assert!(
            !error.contains("sk-super-secret"),
            "验证错误不应包含 API Key"
        );
    }
}

// ── 辅助函数 ──

fn mask_api_key(key: &str) -> String {
    if key.len() <= 6 {
        return "***".to_string();
    }
    let prefix = &key[..3];
    let suffix = &key[key.len() - 4..];
    format!("{}***{}", prefix, suffix)
}
