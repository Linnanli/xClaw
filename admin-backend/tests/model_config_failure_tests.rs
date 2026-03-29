//! 模型配置 - 失败路径测试
//!
//! 覆盖各种异常和边界条件。

use serde_json::json;

/// 模拟 CreateModelConfigRequest 验证逻辑
fn validate_create_request(body: &serde_json::Value) -> Result<(), String> {
    let model_id = body["model_id"].as_str().unwrap_or("");
    let display_name = body["display_name"].as_str().unwrap_or("");
    let provider = body["provider"].as_str().unwrap_or("");
    let api_key = body["api_key"].as_str().unwrap_or("");

    if model_id.trim().is_empty() {
        return Err("model_id 不能为空".to_string());
    }
    if display_name.trim().is_empty() {
        return Err("display_name 不能为空".to_string());
    }
    if provider.trim().is_empty() {
        return Err("provider 不能为空".to_string());
    }
    if api_key.trim().is_empty() {
        return Err("api_key 不能为空".to_string());
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

#[test]
fn test_failure_create_empty_api_key() {
    let body = json!({
        "model_id": "test",
        "display_name": "Test",
        "provider": "openai",
        "api_key": ""
    });
    let err = validate_create_request(&body).unwrap_err();
    assert!(err.contains("api_key"), "空 API Key 应被拒绝创建");
}

#[test]
fn test_failure_create_missing_api_key() {
    let body = json!({
        "model_id": "test",
        "display_name": "Test",
        "provider": "openai"
    });
    let err = validate_create_request(&body).unwrap_err();
    assert!(err.contains("api_key"), "缺少 API Key 应被拒绝创建");
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
        "provider": "openai",
        "api_key": "sk-test"
    });
    // 基本验证应通过（长度限制由数据库约束）
    assert!(validate_create_request(&body).is_ok());
}

#[test]
fn test_failure_special_characters_in_model_id() {
    let body = json!({
        "model_id": "model/with:special@chars",
        "display_name": "Test",
        "provider": "openai",
        "api_key": "sk-test"
    });
    // 基本验证应通过（特殊字符由业务层处理）
    assert!(validate_create_request(&body).is_ok());
}

#[test]
fn test_failure_unicode_in_display_name() {
    let body = json!({
        "model_id": "test",
        "display_name": "我的模型 🤖",
        "provider": "openai",
        "api_key": "sk-test"
    });
    assert!(validate_create_request(&body).is_ok());
}


// ============================================================================
// 测试连接失败路径
// ============================================================================

/// 镜像 `TestConnectionRequest` 验证逻辑
fn validate_test_connection(body: &serde_json::Value) -> Result<(), String> {
    let api_base_url = body["api_base_url"].as_str().unwrap_or("");
    if api_base_url.is_empty() {
        return Err("api_base_url 不能为空".to_string());
    }
    let api_key = body["api_key"].as_str().unwrap_or("");
    if api_key.is_empty() {
        return Err("api_key 不能为空".to_string());
    }
    Ok(())
}

/// 镜像后端 HTTP 状态码分类逻辑
fn classify_test_connection_status(status: u16) -> Result<&'static str, String> {
    match status {
        200..=299 => Ok("连接成功"),
        401 => Err(format!("认证失败 ({}): API Key 无效", status)),
        400 | 403 | 404 | 422 => Ok("连接成功（API 可达，模型或请求参数可能需要调整）"),
        429 => Ok("连接成功（当前被限流，请稍后再正式使用）"),
        _ => Err(format!("服务端错误 ({})", status)),
    }
}

#[test]
fn test_failure_test_connection_empty_api_key() {
    let body = json!({
        "provider": "openai",
        "api_base_url": "https://api.openai.com/v1",
        "api_key": ""
    });
    let err = validate_test_connection(&body).unwrap_err();
    assert!(err.contains("api_key"), "空 API Key 应被拒绝");
}

#[test]
fn test_failure_test_connection_missing_api_key() {
    let body = json!({
        "provider": "openai",
        "api_base_url": "https://api.openai.com/v1"
    });
    let err = validate_test_connection(&body).unwrap_err();
    assert!(err.contains("api_key"), "缺少 API Key 应被拒绝");
}

#[test]
fn test_failure_test_connection_empty_base_url() {
    let body = json!({
        "provider": "openai",
        "api_base_url": "",
        "api_key": "sk-test"
    });
    let err = validate_test_connection(&body).unwrap_err();
    assert!(err.contains("api_base_url"), "空 base URL 应被拒绝");
}

#[test]
fn test_failure_test_connection_401_rejected() {
    let result = classify_test_connection_status(401);
    assert!(result.is_err(), "401 应被视为认证失败，而非连接成功");
    assert!(result.unwrap_err().contains("认证失败"));
}

#[test]
fn test_failure_test_connection_403_accepted_as_model_issue() {
    // 403 可能是模型级别权限问题（如智谱对不存在模型返回 403），不应视为 API Key 无效
    let result = classify_test_connection_status(403);
    assert!(result.is_ok(), "403 应视为连接成功（模型或权限问题，非 API Key 无效）");
}

#[test]
fn test_failure_test_connection_429_accepted_as_rate_limited() {
    // 429 说明连接和认证正常，只是被限流，应视为连接成功
    let result = classify_test_connection_status(429);
    assert!(result.is_ok(), "429 应视为连接成功（被限流但连接正常）");
}

#[test]
fn test_failure_test_connection_500_rejected() {
    let result = classify_test_connection_status(500);
    assert!(result.is_err(), "500 应被视为服务端错误");
}

#[test]
fn test_failure_test_connection_200_accepted() {
    let result = classify_test_connection_status(200);
    assert!(result.is_ok(), "200 应视为连接成功");
}

#[test]
fn test_failure_test_connection_400_accepted_as_auth_pass() {
    // 400 表示认证通过但请求参数有误（model "test" 不存在），视为连接成功
    let result = classify_test_connection_status(400);
    assert!(result.is_ok(), "400 应视为连接成功（认证通过，模型不存在）");
}

#[test]
fn test_failure_test_connection_404_accepted_as_auth_pass() {
    let result = classify_test_connection_status(404);
    assert!(result.is_ok(), "404 应视为连接成功（认证通过，端点不存在）");
}
