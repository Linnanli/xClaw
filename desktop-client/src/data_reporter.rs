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

    /// 将事件加入上报队列。
    ///
    /// 如果队列已满，丢弃最旧的事件（FIFO 淘汰）。
    pub fn enqueue(&self, report: ClientReport) {
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
