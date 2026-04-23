//! 异步审批轮询模块（需求 23）
//!
//! 实现对话流异步审批任务：
//! - 创建审批工单后启动后台轮询任务
//! - 每 30 秒检查工单状态，状态变更时通过 chat-stream 通知前端
//! - 本地持久化 pending ticket_id 列表，重启时恢复轮询
//!
//! # 架构
//!
//! ```text
//! 前端 invoke('submit_approval_ticket')
//!   → POST /api/approvals → 获取 ticket_id
//!   → tokio::spawn(poll_approval_status)
//!     → 每 30 秒 GET /api/approvals/{id}/check（最多 2880 次 = 24h）
//!     → 状态变更 → app_handle.emit("chat-stream", ApprovalResult)
//! ```
//!
//! # 状态共享
//!
//! `PendingTicketStore` 作为 Tauri managed state 注入，所有轮询任务
//! 和 `submit_approval_ticket` 命令共用同一个实例，避免多实例隔离问题。

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

/// 每个 ticket 最多轮询次数（30s × 2880 = 24 小时）
const MAX_POLL_ATTEMPTS: u32 = 2880;

// ── 数据结构 ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PendingTickets {
    pub tickets: Vec<PendingTicket>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingTicket {
    pub ticket_id: String,
    pub thread_id: String,
    #[serde(default)]
    pub request_id: Option<String>,
    pub content: String,
}

impl PendingTickets {
    fn storage_path() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("ironclaw-desktop")
            .join("pending_approvals.json")
    }

    pub fn load() -> Self {
        let path = Self::storage_path();
        if !path.exists() {
            return Self::default();
        }
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save(&self) {
        let path = Self::storage_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(&path, json);
        }
    }

    pub fn add(&mut self, ticket: PendingTicket) {
        let request_id = ticket.request_id.as_deref();
        self.tickets.retain(|current| {
            current.ticket_id != ticket.ticket_id
                && !matches!(request_id, Some(request_id) if current.request_id.as_deref() == Some(request_id))
        });
        self.tickets.push(ticket);
        self.save();
    }

    pub fn ticket_id_for_request_id(&self, request_id: &str) -> Option<String> {
        self.tickets.iter().find_map(|ticket| {
            (ticket.request_id.as_deref() == Some(request_id)).then(|| ticket.ticket_id.clone())
        })
    }

    pub fn remove(&mut self, ticket_id: &str) {
        self.tickets.retain(|t| t.ticket_id != ticket_id);
        self.save();
    }
}

/// Tauri managed state：全局唯一的 pending tickets 存储。
///
/// 通过 `app.manage(PendingTicketStore::init())` 注入，
/// 所有轮询任务和命令通过 `app_handle.state::<PendingTicketStore>()` 获取。
pub struct PendingTicketStore(pub Arc<Mutex<PendingTickets>>);

impl PendingTicketStore {
    /// 从磁盘加载持久化数据并初始化。
    pub fn init() -> Self {
        Self(Arc::new(Mutex::new(PendingTickets::load())))
    }

    /// 获取内部 Arc，用于传递给异步任务。
    pub fn inner(&self) -> Arc<Mutex<PendingTickets>> {
        Arc::clone(&self.0)
    }
}

// ── 轮询任务 ──────────────────────────────────────────────────────

/// 启动单个审批工单的后台轮询任务。
///
/// 每 30 秒调用 `GET /api/approvals/{id}/check`，
/// 检测到状态变更时通过 `chat-stream` 推送结果并终止轮询。
/// 超过 `MAX_POLL_ATTEMPTS` 次后自动终止（防止无限循环）。
pub async fn poll_approval_status(
    app_handle: tauri::AppHandle,
    ticket_id: String,
    thread_id: String,
    request_id: Option<String>,
    store: Arc<Mutex<PendingTickets>>,
) {
    let admin_url =
        std::env::var("ADMIN_BACKEND_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());
    let client_token = std::env::var("ADMIN_AUTH_TOKEN").unwrap_or_default();

    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build();

    let http = match http {
        Ok(client) => client,
        Err(error) => {
            tracing::warn!(error = %error, "Failed to build approval polling client");
            return;
        }
    };

    let url = format!("{}/api/approvals/{}/check", admin_url, ticket_id);
    let mut interval = tokio::time::interval(Duration::from_secs(30));
    let mut attempts = 0u32;

    loop {
        interval.tick().await;
        attempts += 1;

        if attempts > MAX_POLL_ATTEMPTS {
            tracing::warn!(
                ticket_id = %ticket_id,
                "Approval polling exceeded max attempts ({}), stopping",
                MAX_POLL_ATTEMPTS
            );
            store.lock().await.remove(&ticket_id);
            return;
        }

        let result = http.get(&url).bearer_auth(&client_token).send().await;

        match result {
            Ok(resp) if resp.status().is_success() => {
                let Ok(data) = resp.json::<serde_json::Value>().await else {
                    continue;
                };
                let status = data["status"].as_str().unwrap_or("pending");
                match status {
                    "approved" | "rejected" | "expired" => {
                        emit_approval_result(
                            &app_handle,
                            &ticket_id,
                            &thread_id,
                            request_id.as_deref(),
                            status,
                            &data,
                        );
                        store.lock().await.remove(&ticket_id);
                        tracing::info!(
                            ticket_id = %ticket_id,
                            status = %status,
                            "Approval polling completed"
                        );
                        return;
                    }
                    _ => {
                        tracing::debug!(ticket_id = %ticket_id, "Approval still pending");
                    }
                }
            }
            Ok(resp) if resp.status().as_u16() == 404 => {
                tracing::warn!(ticket_id = %ticket_id, "Approval ticket not found, stopping poll");
                store.lock().await.remove(&ticket_id);
                return;
            }
            Err(e) => {
                tracing::debug!(
                    error = %e,
                    ticket_id = %ticket_id,
                    "Approval poll network error, retrying"
                );
            }
            _ => {}
        }
    }
}

fn emit_approval_result(
    app_handle: &tauri::AppHandle,
    ticket_id: &str,
    thread_id: &str,
    request_id: Option<&str>,
    status: &str,
    data: &serde_json::Value,
) {
    let event = crate::vercel_ui_protocol::VercelUIStream::DataCustom {
        id: None,
        data: serde_json::json!({
            "type": "approval_result",
            "ticket_id": ticket_id,
            "thread_id": thread_id,
            "request_id": request_id,
            "status": status,
            "review_comment": data.get("review_comment").and_then(|v| v.as_str()),
            "expires_at": data.get("expires_at").and_then(|v| v.as_str()),
        }),
    };
    // 线程专属：审批结果归属发起该审批的 thread
    let thread_scope = if thread_id.is_empty() {
        None
    } else {
        Some(thread_id)
    };
    let _ = crate::tauri_channel::emit_chat_stream(app_handle, thread_scope, &event);
}

#[cfg(test)]
mod tests {
    use super::{PendingTicket, PendingTickets};

    #[test]
    fn test_pending_ticket_request_id_is_optional_for_backward_compat() {
        let legacy = r#"{"ticket_id":"t1","thread_id":"thread-1","content":"desc"}"#;
        let ticket: PendingTicket = serde_json::from_str(legacy).expect("legacy ticket should deserialize");
        assert!(ticket.request_id.is_none());
    }

    #[test]
    fn test_pending_ticket_serializes_request_id() {
        let ticket = PendingTicket {
            ticket_id: "t1".into(),
            thread_id: "thread-1".into(),
            request_id: Some("req-1".into()),
            content: "desc".into(),
        };

        let json = serde_json::to_value(&ticket).expect("ticket should serialize");
        assert_eq!(json["request_id"], "req-1");
    }

    #[test]
    fn test_pending_tickets_add_replaces_existing_request_id() {
        let mut tickets = PendingTickets::default();
        tickets.add(PendingTicket {
            ticket_id: "ticket-old".into(),
            thread_id: "thread-1".into(),
            request_id: Some("req-1".into()),
            content: "first".into(),
        });

        tickets.add(PendingTicket {
            ticket_id: "ticket-new".into(),
            thread_id: "thread-1".into(),
            request_id: Some("req-1".into()),
            content: "second".into(),
        });

        assert_eq!(tickets.tickets.len(), 1);
        assert_eq!(tickets.tickets[0].ticket_id, "ticket-new");
    }

    #[test]
    fn test_ticket_id_for_request_id_returns_existing_ticket() {
        let mut tickets = PendingTickets::default();
        tickets.add(PendingTicket {
            ticket_id: "ticket-1".into(),
            thread_id: "thread-1".into(),
            request_id: Some("req-1".into()),
            content: "first".into(),
        });

        let ticket_id = tickets.ticket_id_for_request_id("req-1");

        assert_eq!(ticket_id.as_deref(), Some("ticket-1"));
    }

    #[test]
    fn test_restore_snapshot_keeps_latest_ticket_per_request() {
        let mut tickets = PendingTickets::default();
        tickets.add(PendingTicket {
            ticket_id: "ticket-old".into(),
            thread_id: "thread-1".into(),
            request_id: Some("req-restore".into()),
            content: "first".into(),
        });
        tickets.add(PendingTicket {
            ticket_id: "ticket-new".into(),
            thread_id: "thread-1".into(),
            request_id: Some("req-restore".into()),
            content: "second".into(),
        });

        let restored_snapshot = tickets.tickets.clone();

        assert_eq!(restored_snapshot.len(), 1);
        assert_eq!(restored_snapshot[0].ticket_id, "ticket-new");
    }
}
