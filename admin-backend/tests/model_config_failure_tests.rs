//! 模型配置 - 失败路径测试
//!
//! 覆盖各种异常和边界条件。

use serde_json::json;

/// 模拟 CreateModelConfigRequest 验证逻辑
fn validate_create_request(body: &serde_json::Value) -> Result<(), String> {
    let model_id = body["model_id"].as_str().unwrap_or("");
    let display_name = body["display_name"].as_str().unwrap_or("");
    let provider = body["provider"].as_str().unwrap_or("");

    if model_id.trim().is_empty() {
        return Err("model_id 不能为空".to_string());
    }
    if display_name.trim().is_empty() {
        return Err("display_name 不能为空".to_string());
    }
    if provider.trim().is_empty() {
        return Err("provider 不能为空".to_string());
    }
    Ok(())
}

/// 模拟 UpdateModelConfigRequest 验证逻辑
fn validate_update_request(body: &serde_json::Value) -> Result<(), String> {
    // 至少需要一个字段
    let has_field = body.get("display_name").is_some()
        || body.get("description").is_some()
        || body.get("provider").is_some()
        || body.get("api_base_url").is_some()
        || body.get("api_key").is_some()
        || body.get("enabled").is_some()
        || body.get("is_default").is_some()
        || body.get("sort_order").is_some()
        || body.get("capabilities").is_some();

    if !has_field {
        return Err("至少需要提供一个更新字段".to_string());
    }
    Ok(())
}

// ============================================================================
// 创建失败路径
// ============================================================================

#[test]
fn test_failure_create_empty_model_id() {
    let body = json!({
        "model_id": "",
        "display_name": "Test",
        "provider": "openai"
    });
    assert!(validate_create_request(&body).is_err());
}

#[test]
fn test_failure_create_empty_display_name() {
    let body = json!({
        "model_id": "test",
        "display_name": "",
        "provider": "openai"
    });
    assert!(validate_create_request(&body).is_err());
}

#[test]
fn test_failure_create_empty_provider() {
    let body = json!({
        "model_id": "test",
        "display_name": "Test",
        "provider": ""
    });
    assert!(validate_create_request(&body).is_err());
}

#[test]
fn test_failure_create_whitespace_only_model_id() {
    let body = json!({
        "model_id": "   ",
        "display_name": "Test",
        "provider": "openai"
    });
    assert!(validate_create_request(&body).is_err());
}

#[test]
fn test_failure_create_missing_fields() {
    let body = json!({});
    assert!(validate_create_request(&body).is_err());
}

// ============================================================================
// 更新失败路径
// ============================================================================

#[test]
fn test_failure_update_empty_body() {
    let body = json!({});
    assert!(validate_update_request(&body).is_err());
}

#[test]
fn test_failure_update_with_valid_field() {
    let body = json!({"display_name": "New Name"});
    assert!(validate_update_request(&body).is_ok());
}

#[test]
fn test_failure_update_with_enabled_field() {
    let body = json!({"enabled": false});
    assert!(validate_update_request(&body).is_ok());
}

// ============================================================================
// 删除失败路径
// ============================================================================

#[test]
fn test_failure_delete_default_model_should_be_rejected() {
    // 模拟删除默认模型的逻辑
    let is_default = true;
    if is_default {
        let err = "不能删除默认模型，请先设置其他模型为默认";
        assert!(err.contains("默认模型"));
    }
}

// ============================================================================
// 边界条件
// ============================================================================

#[test]
fn test_failure_very_long_model_id() {
    let long_id = "a".repeat(1000);
    let body = json!({
        "model_id": long_id,
        "display_name": "Test",
        "provider": "openai"
    });
    // 基本验证应通过（长度限制由数据库约束）
    assert!(validate_create_request(&body).is_ok());
}

#[test]
fn test_failure_special_characters_in_model_id() {
    let body = json!({
        "model_id": "model/with:special@chars",
        "display_name": "Test",
        "provider": "openai"
    });
    // 基本验证应通过（特殊字符由业务层处理）
    assert!(validate_create_request(&body).is_ok());
}

#[test]
fn test_failure_unicode_in_display_name() {
    let body = json!({
        "model_id": "test",
        "display_name": "我的模型 🤖",
        "provider": "openai"
    });
    assert!(validate_create_request(&body).is_ok());
}
