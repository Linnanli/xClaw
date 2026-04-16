//! 数据上报模块。
//!
//! 收集客户端运行数据（审计日志、DLP 事件、使用统计、健康状态），
//! 批量上报到 Admin Backend。支持离线缓冲和失败重试。
//!
//! # 架构
//!
//! ```text
//! Desktop Client                   Admin Backend
//! ┌──────────────────────┐        ┌──────────────────┐
//! │ DataReporter          │        │                  │
//! │  ├── 审计日志收集     │──POST──►│ POST /api/       │
//! │  ├── DLP 事件收集     │        │ client-reports   │
//! │  ├── 使用统计收集     │        │                  │
//! │  ├── 本地队列缓冲     │        │ 存入数据库       │
//! │  └── 批量上报(30s)   │        │ 管理端可查看     │
//! └──────────────────────┘        └──────────────────┘
//! ```
//!
//! # 安全
//!
//! - 不上报原始消息内容
//! - DLP 事件只上报统计信息，不上报匹配的原始数据
//! - 使用 HTTPS + Bearer Token 认证

use std::sync::Mutex;
use std::time::Duration;

use base64::Engine;
use serde::{Deserialize, Serialize};

/// 客户端上报事件。
///
/// 所有事件类型使用 `#[serde(tag = "type")]` 标记，
/// 便于 Admin Backend 按类型分发处理。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClientReport {
    /// 审计日志：用户操作记录。
    #[serde(rename = "audit_log")]
    AuditLog {
        timestamp: String,
        action: String,
        details: serde_json::Value,
    },

    /// DLP 事件：敏感数据检测统计。
    ///
    /// ⚠️ 不上报原始内容，只上报统计信息。
    #[serde(rename = "dlp_event")]
    DlpEvent {
        timestamp: String,
        had_sensitive_data: bool,
        was_blocked: bool,
        rule_matches: Vec<String>,
    },

    /// 使用统计：LLM 调用量、token 消耗等。
    #[serde(rename = "usage_stats")]
    UsageStats {
        timestamp: String,
        period_start: String,
        period_end: String,
        total_messages: u64,
        total_tokens: u64,
        total_cost_cents: u64,
    },

    /// 健康状态：客户端运行状态。
    #[serde(rename = "health_status")]
    HealthStatus {
        timestamp: String,
        client_version: String,
        uptime_secs: u64,
        active_extensions: Vec<String>,
    },

    /// 对话记录：AI 对话审计。
    ///
    /// 对话结束时上报，包含完整的消息流和 Token 消耗。
    /// 后端用于对话审计和费用统计。
    ///
    /// 注意：后端会从 messages 中提取 Token 信息并自动调用费用上报接口，
    /// 因此客户端无需单独调用 `report_usage_to_admin()`。
    #[serde(rename = "conversation")]
    Conversation {
        /// 客户端生成的对话 ID（用于幂等去重）
        client_conversation_id: String,
        /// 用户 ID
        user_id: String,
        /// 对话主题/标题
        #[serde(skip_serializing_if = "Option::is_none")]
        topic: Option<String>,
        /// 使用的模型 ID
        #[serde(skip_serializing_if = "Option::is_none")]
        model_id: Option<String>,
        /// 是否被 DLP 标记
        #[serde(skip_serializing_if = "Option::is_none")]
        dlp_flagged: Option<bool>,
        /// DLP 详情
        #[serde(skip_serializing_if = "Option::is_none")]
        dlp_details: Option<String>,
        /// 本次对话中激活过的技能名（去重后）
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        used_skills: Vec<String>,
        /// 消息列表
        messages: Vec<ConversationMessage>,
    },
}

/// 对话消息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMessage {
    /// 角色：user / assistant / system
    pub role: String,
    /// 消息内容
    pub content: String,
    /// 附件元数据（仅用户消息通常有值）
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<ConversationAttachment>,
    /// 使用的模型 ID（仅 assistant 消息有值）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    /// 输入 Token 数（仅 assistant 消息有值）
    #[serde(default)]
    pub input_tokens: i32,
    /// 输出 Token 数（仅 assistant 消息有值）
    #[serde(default)]
    pub output_tokens: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConversationAttachment {
    pub id: String,
    pub kind: String,
    pub mime_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extracted_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_data_base64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_secs: Option<u32>,
}

impl ConversationAttachment {
    pub fn from_frontend(
        id: String,
        kind: String,
        mime_type: String,
        filename: Option<String>,
        size_bytes: Option<u64>,
        extracted_text: Option<String>,
        data: &[u8],
        duration_secs: Option<u32>,
    ) -> Self {
        let image_data_base64 = if kind == "image" && !data.is_empty() {
            Some(base64::engine::general_purpose::STANDARD.encode(data))
        } else {
            None
        };

        Self {
            id,
            kind,
            mime_type,
            filename,
            size_bytes,
            extracted_text,
            image_data_base64,
            duration_secs,
        }
    }
}

#[derive(Clone)]
struct HealthStatusSnapshot {
    client_version: String,
    active_extensions: Vec<String>,
    reported_at: std::time::Instant,
}

const DEFAULT_HEALTH_STATUS_MIN_INTERVAL_SECS: u64 = 10 * 60;

/// 数据上报器。
///
/// 使用内存队列缓冲事件，定期批量上报到 Admin Backend。
/// 上报失败时事件保留在队列中，下次重试。
pub struct DataReporter {
    /// 事件队列（线程安全）
    queue: Mutex<Vec<ClientReport>>,
    /// Admin Backend URL
    admin_url: String,
    /// 客户端认证 token
    client_token: String,
    /// HTTP 客户端
    http_client: reqwest::Client,
    /// 上报间隔
    flush_interval: Duration,
    /// 队列最大容量（防止内存溢出）
    max_queue_size: usize,
    /// 相同健康状态最小上报间隔。
    health_status_min_interval: Duration,
    /// 最近一次健康状态快照，用于降采样。
    last_health_status: Mutex<Option<HealthStatusSnapshot>>,
}

impl DataReporter {
    /// 创建新的数据上报器。
    pub fn new(admin_url: String, client_token: String) -> Self {
        Self {
            queue: Mutex::new(Vec::new()),
            admin_url,
            client_token,
            http_client: reqwest::Client::builder()
                .timeout(Duration::from_secs(15))
                .build()
                .unwrap_or_default(),
            flush_interval: Duration::from_secs(30),
            max_queue_size: 10_000,
            health_status_min_interval: Duration::from_secs(
                DEFAULT_HEALTH_STATUS_MIN_INTERVAL_SECS,
            ),
            last_health_status: Mutex::new(None),
        }
    }

    /// 设置上报间隔。
    pub fn with_flush_interval(mut self, interval: Duration) -> Self {
        self.flush_interval = interval;
        self
    }

    /// 设置队列最大容量。
    pub fn with_max_queue_size(mut self, size: usize) -> Self {
        self.max_queue_size = size;
        self
    }

    /// 设置相同 health_status 的最小上报间隔。
    pub fn with_health_status_min_interval(mut self, interval: Duration) -> Self {
        self.health_status_min_interval = interval;
        self
    }

    /// 将事件加入上报队列。
    ///
    /// 如果队列已满，丢弃最旧的事件（FIFO 淘汰）。
    pub fn enqueue(&self, report: ClientReport) {
        if !self.should_enqueue(&report) {
            return;
        }

        let mut queue = match self.queue.lock() {
            Ok(q) => q,
            Err(poisoned) => {
                tracing::warn!("Report queue lock poisoned, recovering");
                poisoned.into_inner()
            }
        };

        if queue.len() >= self.max_queue_size {
            // 丢弃最旧的 10% 事件
            let drain_count = self.max_queue_size / 10;
            queue.drain(..drain_count);
            tracing::warn!(
                dropped = drain_count,
                "Report queue full, dropped oldest events"
            );
        }

        queue.push(report);
    }

    fn should_enqueue(&self, report: &ClientReport) -> bool {
        match report {
            ClientReport::HealthStatus {
                client_version,
                active_extensions,
                ..
            } => self.should_enqueue_health_status(client_version, active_extensions),
            _ => true,
        }
    }

    fn should_enqueue_health_status(
        &self,
        client_version: &str,
        active_extensions: &[String],
    ) -> bool {
        let mut normalized_extensions = active_extensions.to_vec();
        normalized_extensions.sort();

        let now = std::time::Instant::now();
        let mut last = match self.last_health_status.lock() {
            Ok(v) => v,
            Err(poisoned) => poisoned.into_inner(),
        };

        if let Some(snapshot) = last.as_ref() {
            let same_status = snapshot.client_version == client_version
                && snapshot.active_extensions == normalized_extensions;
            let within_interval =
                now.duration_since(snapshot.reported_at) < self.health_status_min_interval;
            if same_status && within_interval {
                return false;
            }
        }

        *last = Some(HealthStatusSnapshot {
            client_version: client_version.to_string(),
            active_extensions: normalized_extensions,
            reported_at: now,
        });
        true
    }

    /// 获取当前队列长度。
    pub fn queue_len(&self) -> usize {
        self.queue.lock().map(|q| q.len()).unwrap_or(0)
    }

    /// 取出队列中所有事件（仅用于测试）。
    #[cfg(test)]
    pub fn drain_for_test(&self) -> Vec<ClientReport> {
        let mut queue = match self.queue.lock() {
            Ok(q) => q,
            Err(p) => p.into_inner(),
        };
        std::mem::take(&mut *queue)
    }

    /// 执行一次批量上报。
    ///
    /// 从队列中取出所有事件，发送到 Admin Backend。
    /// 如果上报失败，事件放回队列。
    pub async fn flush(&self) -> Result<usize, String> {
        let reports: Vec<ClientReport> = {
            let mut queue = self
                .queue
                .lock()
                .map_err(|e| format!("Queue lock poisoned: {}", e))?;
            std::mem::take(&mut *queue)
        };

        if reports.is_empty() {
            return Ok(0);
        }

        let count = reports.len();
        let url = format!("{}/api/client-reports", self.admin_url);

        match self
            .http_client
            .post(&url)
            .bearer_auth(&self.client_token)
            .json(&reports)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                tracing::debug!(count = count, "Reported events to admin");
                Ok(count)
            }
            Ok(resp) => {
                let status = resp.status();
                // 上报失败，放回队列
                self.requeue(reports);
                Err(format!("Server returned {}", status))
            }
            Err(e) => {
                // 网络错误，放回队列
                self.requeue(reports);
                Err(format!("HTTP request failed: {}", e))
            }
        }
    }

    /// 将事件放回队列头部（保持顺序）。
    fn requeue(&self, reports: Vec<ClientReport>) {
        let mut queue = match self.queue.lock() {
            Ok(q) => q,
            Err(poisoned) => poisoned.into_inner(),
        };
        // 将失败的事件插入队列头部，新事件在尾部
        let mut combined = reports;
        combined.append(&mut *queue);
        *queue = combined;
    }

    /// 启动后台上报循环。
    ///
    /// 定期执行 `flush()`。失败时静默重试，不影响客户端运行。
    pub async fn run_flush_loop(&self) {
        let mut interval = tokio::time::interval(self.flush_interval);

        loop {
            interval.tick().await;

            match self.flush().await {
                Ok(0) => {} // 队列为空，静默
                Ok(n) => tracing::debug!(count = n, "Flush completed"),
                Err(e) => tracing::debug!("Flush failed (will retry): {}", e),
            }
        }
    }
}
