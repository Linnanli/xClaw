//! Tauri 命令 — 仅包含需要在 IPC 层直接暴露的命令。
//!
//! 大部分命令已迁移到 `ipc/` 模块，此文件仅保留 `get_auth_token`。

use crate::auth_token_manager::AuthTokenManager;
use crate::{Error, Result};

/// 获取认证令牌。
///
/// 从本地文件加载或自动生成 64 位十六进制 token，
/// 并验证格式合法性后返回给前端。
#[tauri::command]
pub async fn get_auth_token() -> Result<String> {
    let token_manager = AuthTokenManager::new();
    let token = token_manager
        .load_or_generate()
        .map_err(|e| Error::ConfigError(e.to_string()))?;

    if token.len() != 64 {
        tracing::error!("Token length is {} (expected 64)", token.len());
        return Err(Error::ConfigError(format!(
            "Invalid token length: {}",
            token.len()
        )));
    }

    if !token.chars().all(|c| c.is_ascii_hexdigit()) {
        tracing::error!("Token contains non-hex characters");
        return Err(Error::ConfigError(
            "Token contains non-hex characters".to_string(),
        ));
    }

    Ok(token)
}

/// 获取水印配置（从 Admin Backend API 读取）。
///
/// 通过 HTTP 调用 `/api/settings` 接口，提取水印相关字段返回给前端。
/// Admin Backend 不可用时返回默认值。
#[tauri::command]
pub async fn get_watermark_config() -> Result<serde_json::Value> {
    let admin_url = std::env::var("ADMIN_BACKEND_URL")
        .unwrap_or_else(|_| "http://localhost:3000".to_string());

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| Error::ConfigError(e.to_string()))?;

    let mut request = client.get(format!("{}/api/settings", admin_url));
    if let Ok(token) = std::env::var("ADMIN_AUTH_TOKEN") {
        request = request.header("Authorization", format!("Bearer {}", token));
    }

    let response = request.send().await
        .map_err(|e| Error::ConfigError(format!("Failed to fetch settings: {}", e)))?;

    let settings: serde_json::Value = response.json().await
        .map_err(|e| Error::ConfigError(format!("Failed to parse settings: {}", e)))?;

    Ok(serde_json::json!({
        "watermark_enabled": settings.get("watermark_enabled").and_then(|v| v.as_bool()).unwrap_or(false),
        "watermark_template": settings.get("watermark_template").and_then(|v| v.as_str()).unwrap_or("{username} | {datetime}"),
        "watermark_font_size": settings.get("watermark_font_size").and_then(|v| v.as_i64()).unwrap_or(16),
        "watermark_opacity": settings.get("watermark_opacity").and_then(|v| v.as_f64()).unwrap_or(0.1),
        "watermark_position": settings.get("watermark_position").and_then(|v| v.as_str()).unwrap_or("diagonal"),
        "watermark_color": settings.get("watermark_color").and_then(|v| v.as_str()).unwrap_or("#000000"),
    }))
}
