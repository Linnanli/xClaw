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

/// 提交审批工单并启动后台轮询任务（需求 23.10）。
///
/// 调用 `POST /api/approvals` 创建工单，通过 `tokio::spawn` 启动独立后台轮询，
/// 不阻塞对话线程。返回 ticket_id 给前端。
///
/// `store` 从 Tauri managed state 注入，确保所有轮询任务共用同一个持久化实例。
#[tauri::command]
pub async fn submit_approval_ticket(
    app_handle: tauri::AppHandle,
    store: tauri::State<'_, crate::approval_polling::PendingTicketStore>,
    content: String,
    thread_id: String,
) -> Result<String> {
    // 在进入 async 前提取 Arc，避免 State 生命周期跨 await 的问题
    let store_arc = store.0.clone();
    let admin_url = std::env::var("ADMIN_BACKEND_URL")
        .unwrap_or_else(|_| "http://localhost:3000".to_string());
    let client_token = std::env::var("ADMIN_AUTH_TOKEN").unwrap_or_default();

    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| Error::ConfigError(e.to_string()))?;

    let payload = serde_json::json!({
        "operation_type": "conversation_submit",
        "operation_name": content.chars().take(100).collect::<String>(),
        "reason": content,
        "applicant_id": "00000000-0000-0000-0000-000000000000",
    });

    let resp = http
        .post(format!("{}/api/approvals", admin_url))
        .bearer_auth(&client_token)
        .json(&payload)
        .send()
        .await
        .map_err(|e| Error::ConfigError(format!("Failed to create approval: {}", e)))?;

    if !resp.status().is_success() {
        return Err(Error::ConfigError(format!("Server returned {}", resp.status())));
    }

    let data: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| Error::ConfigError(format!("Failed to parse response: {}", e)))?;

    let ticket_id = data["id"]
        .as_str()
        .ok_or_else(|| Error::ConfigError("Missing ticket id in response".into()))?
        .to_string();

    // 持久化到共享 store（managed state，全局唯一）
    store_arc.lock().await.add(crate::approval_polling::PendingTicket {
        ticket_id: ticket_id.clone(),
        thread_id: thread_id.clone(),
        content: content.chars().take(200).collect(),
    });

    // 启动后台轮询任务，传入共享 store 的 Arc
    let store_arc2 = store_arc.clone();
    tokio::spawn(crate::approval_polling::poll_approval_status(
        app_handle,
        ticket_id.clone(),
        thread_id.clone(),
        store_arc2,
    ));

    tracing::info!(
        ticket_id = %ticket_id,
        thread_id = %thread_id,
        "Approval ticket submitted, polling started"
    );

    Ok(ticket_id)
}
