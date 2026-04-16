//! Tauri 命令 — 仅包含需要在 IPC 层直接暴露的命令。
//!
//! 大部分命令已迁移到 `ipc/` 模块，此文件仅保留 `get_auth_token`。

use crate::auth_token_manager::AuthTokenManager;
use crate::{Error, Result};
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Serialize)]
struct ApprovalTicketPayload {
    applicant_id: Uuid,
    operation_type: String,
    operation_name: String,
    reason: Option<String>,
}

/// 构建带超时的 HTTP 客户端。
fn build_http_client(timeout_secs: u64) -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .build()
        .map_err(|e| Error::ConfigError(e.to_string()))
}

/// 从环境变量读取 Admin Backend 地址和认证 token。
fn admin_env() -> (String, String) {
    let url =
        std::env::var("ADMIN_BACKEND_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());
    let token = std::env::var("ADMIN_AUTH_TOKEN").unwrap_or_default();
    (url, token)
}

fn resolve_applicant_id_value(backend_user_id: &std::sync::RwLock<Option<Uuid>>) -> Result<Uuid> {
    crate::state::require_backend_user_id_value(backend_user_id, "审批申请人身份")
        .map_err(Error::ConfigError)
}

fn resolve_applicant_id(state: &crate::state::AppState) -> Result<Uuid> {
    resolve_applicant_id_value(&state.backend_user_id)
}

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

/// 获取应用版本号。
///
/// 从 Cargo.toml 编译期注入的 `CARGO_PKG_VERSION` 读取，格式为 `x.y.z`。
#[tauri::command]
pub fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// 检查是否需要升级（需求 8.9 / 23.3）。
///
/// 调用 `GET /api/client-config` 读取 `needs_upgrade` 字段。
/// Admin Backend 不可用时返回 `false`（不强制升级，Fail-Safe 对用户友好）。
#[tauri::command]
pub async fn check_for_updates() -> Result<serde_json::Value> {
    let (admin_url, client_token) = admin_env();
    let http = build_http_client(5)?;

    // client_id 参数让后端返回该客户端的 needs_upgrade 状态
    let url = if client_token.is_empty() {
        format!("{}/api/client-config", admin_url)
    } else {
        format!("{}/api/client-config?client_id={}", admin_url, client_token)
    };

    let config: serde_json::Value = http
        .get(&url)
        .bearer_auth(&client_token)
        .send()
        .await
        .map_err(|_| Error::ConfigError("Admin Backend 不可用".into()))?
        .json()
        .await
        .map_err(|e| Error::ConfigError(format!("Failed to parse config: {}", e)))?;

    let needs_upgrade = config
        .get("needs_upgrade")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    Ok(serde_json::json!({
        "needs_upgrade": needs_upgrade,
        "current_version": env!("CARGO_PKG_VERSION"),
    }))
}

/// 获取水印配置（从 Admin Backend API 读取）。
///
/// 通过 HTTP 调用 `/api/settings` 接口，提取水印相关字段返回给前端。
/// Admin Backend 不可用时返回默认值。
#[tauri::command]
pub async fn get_watermark_config() -> Result<serde_json::Value> {
    let (admin_url, token) = admin_env();
    let http = build_http_client(5)?;

    let settings: serde_json::Value = http
        .get(format!("{}/api/settings", admin_url))
        .bearer_auth(&token)
        .send()
        .await
        .map_err(|e| Error::ConfigError(format!("Failed to fetch settings: {}", e)))?
        .json()
        .await
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
    state: tauri::State<'_, crate::state::EngineState>,
    store: tauri::State<'_, crate::approval_polling::PendingTicketStore>,
    request_id: String,
    tool_name: String,
    content: String,
    thread_id: String,
) -> Result<String> {
    // 在进入 async 前提取 Arc，避免 State 生命周期跨 await 的问题
    let state = state
        .get()
        .map_err(Error::ConfigError)?;
    let store_arc = store.0.clone();
    let (admin_url, client_token) = admin_env();
    if client_token.is_empty() {
        return Err(Error::ConfigError("未配置 ADMIN_AUTH_TOKEN，无法提交审批工单".into()));
    }

    {
        let store = store_arc.lock().await;
        if let Some(existing_ticket_id) = store.ticket_id_for_request_id(&request_id) {
            tracing::info!(
                ticket_id = %existing_ticket_id,
                request_id = %request_id,
                thread_id = %thread_id,
                "Approval ticket already pending, reusing existing ticket"
            );
            return Ok(existing_ticket_id);
        }
    }

    let applicant_id = resolve_applicant_id(state)?;
    let http = build_http_client(10)?;
    let operation_name = if tool_name.trim().is_empty() {
        "工具审批".to_string()
    } else {
        format!("工具审批: {}", tool_name.trim())
    };
    let reason = format!("request_id={request_id}\nthread_id={thread_id}\n{content}");

    let payload = ApprovalTicketPayload {
        applicant_id,
        operation_type: "tool_approval".to_string(),
        operation_name,
        reason: Some(reason),
    };

    let resp = http
        .post(format!("{}/api/approvals", admin_url))
        .bearer_auth(&client_token)
        .json(&payload)
        .send()
        .await
        .map_err(|e| Error::ConfigError(format!("Failed to create approval: {}", e)))?;

    if !resp.status().is_success() {
        return Err(Error::ConfigError(format!(
            "Server returned {}",
            resp.status()
        )));
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
    store_arc
        .lock()
        .await
        .add(crate::approval_polling::PendingTicket {
            ticket_id: ticket_id.clone(),
            thread_id: thread_id.clone(),
            request_id: Some(request_id.clone()),
            content: content.chars().take(200).collect(),
        });

    // 启动后台轮询任务，传入共享 store 的 Arc
    let store_arc2 = store_arc.clone();
    tokio::spawn(crate::approval_polling::poll_approval_status(
        app_handle,
        ticket_id.clone(),
        thread_id.clone(),
        Some(request_id.clone()),
        store_arc2,
    ));

    tracing::info!(
        ticket_id = %ticket_id,
        request_id = %request_id,
        thread_id = %thread_id,
        "Approval ticket submitted, polling started"
    );

    Ok(ticket_id)
}

#[cfg(test)]
mod tests {
    use super::resolve_applicant_id_value;
    use crate::Error;
    use std::sync::RwLock;
    use uuid::Uuid;

    #[test]
    fn test_resolve_applicant_id_value_returns_backend_uuid() {
        let applicant_id =
            Uuid::parse_str("550e8400-e29b-41d4-a716-446655440123").expect("uuid should parse");
        let backend_user_id = RwLock::new(Some(applicant_id));

        let resolved =
            resolve_applicant_id_value(&backend_user_id).expect("ready backend identity should resolve");

        assert_eq!(resolved, applicant_id);
    }

    #[test]
    fn test_resolve_applicant_id_value_fails_when_missing() {
        let backend_user_id = RwLock::new(None);

        let error = resolve_applicant_id_value(&backend_user_id)
            .expect_err("missing backend identity should block approval submission");

        match error {
            Error::ConfigError(message) => {
                assert_eq!(message, "审批申请人身份：后台用户身份尚未就绪");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn test_resolve_applicant_id_value_fails_when_lock_poisoned() {
        let backend_user_id = RwLock::new(Some(Uuid::nil()));
        let _ = std::panic::catch_unwind(|| {
            let _guard = backend_user_id.write().expect("write lock should succeed");
            panic!("poison applicant identity lock");
        });

        let error = resolve_applicant_id_value(&backend_user_id)
            .expect_err("poisoned backend identity should block approval submission");

        match error {
            Error::ConfigError(message) => {
                assert_eq!(message, "审批申请人身份：后台用户身份读取失败");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
