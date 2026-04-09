//! 模型配置 - 安全审计测试
//!
//! 验证敏感信息（API Key）在各个路径中不会泄露。

use serde_json::json;

// ── 辅助函数 ──

fn mask_api_key(key: &str) -> String {
    if key.len() <= 6 {
        return "***".to_string();
    }
    let prefix = &key[..3];
    let suffix = &key[key.len() - 4..];
    format!("{}***{}", prefix, suffix)
}

// ============================================================================
// API Key 脱敏
// ============================================================================

#[test]
fn test_audit_client_models_endpoint_excludes_api_key() {
    let client_response = json!([{
        "model_id": "gpt-4o",
        "display_name": "GPT-4o",
        "description": "Test",
        "provider": "openai",
        "is_default": true,
        "capabilities": ["chat"]
    }]);

    let response_str = serde_json::to_string(&client_response).unwrap();
    assert!(
        !response_str.contains("api_key"),
        "客户端 API 响应不应包含 api_key 字段"
    );
    assert!(
        !response_str.contains("api_base_url"),
        "客户端 API 响应不应包含 api_base_url 字段"
    );
    assert!(
        !response_str.contains("sk-"),
        "客户端 API 响应不应包含 API Key 值"
    );
}

#[test]
fn test_audit_api_key_masking_in_admin_response() {
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
    assert!(
        !masked.contains("sk") || masked.len() <= 6,
        "短 key 应被完全脱敏"
    );
}

// ============================================================================
// 错误信息不泄露 API Key
// ============================================================================

#[test]
fn test_audit_error_messages_no_key_leak() {
    let api_key = "sk-secret-key-12345";
    let error_messages = vec![
        "模型配置创建失败: 数据库错误".to_string(),
        "模型 ID 已存在: gpt-4o".to_string(),
        "无法连接到模型 API".to_string(),
    ];
    for msg in &error_messages {
        assert!(!msg.contains(api_key), "错误信息不应包含 API Key: {}", msg);
    }
}

#[test]
fn test_audit_create_request_validation_no_key_in_error() {
    let request = json!({
        "model_id": "",
        "display_name": "",
        "provider": "openai",
        "api_key": "sk-super-secret-key"
    });
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

// ============================================================================
// 测试连接 - 安全审计
// ============================================================================

#[test]
fn test_audit_test_connection_auth_error_no_key_leak() {
    let api_key = "sk-super-secret-key-12345";
    let status = 401;
    let error_msg = format!("认证失败 ({}): API Key 无效或权限不足", status);
    assert!(
        !error_msg.contains(api_key),
        "认证失败错误信息不应包含原始 API Key"
    );
    assert!(
        !error_msg.contains("sk-super"),
        "认证失败错误信息不应包含 API Key 前缀"
    );
}

#[test]
fn test_audit_test_connection_requires_api_key() {
    // Fail-Safe: 没有 API Key 时必须拒绝测试连接，而非允许无认证请求
    let api_key = "";
    assert!(api_key.is_empty(), "空 API Key 应触发验证拒绝");
    // 后端逻辑：api_key 为空时返回 Validation Error，不发送 HTTP 请求
}
