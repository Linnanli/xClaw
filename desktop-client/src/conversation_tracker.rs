//! 对话追踪器 — 收集对话消息并在对话结束时上报到 Admin Backend。
//!
//! # 架构
//!
//! ```text
//! send_chat_message()
//!   → ConversationTracker::record_user_message()
//!
//! TauriChannel::respond()
//!   → ConversationTracker::record_assistant_message()
//!   → 若对话超时或显式结束 → flush_conversation()
//!     → DataReporter::enqueue(ClientReport::Conversation)
//! ```
//!
//! # 设计决策
//!
//! - 按 thread_id 分组，每个 thread 维护独立的消息缓冲
//! - 超过 10 分钟无新消息视为对话结束，自动 flush
//! - 显式切换 thread 时 flush 旧 thread
//! - 上报失败不影响主流程（DataReporter 有重试机制）

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use uuid::Uuid;

use crate::data_reporter::{
    ClientReport, ConversationAttachment, ConversationMessage, DataReporter,
};

/// 单个 thread 的对话缓冲。
struct ThreadBuffer {
    report_id: Uuid,
    messages: Vec<ConversationMessage>,
    last_activity: Instant,
    /// 本次对话使用的主模型（最后一条 assistant 消息的 model_id）
    model_id: Option<String>,
    /// 是否有 DLP 标记
    dlp_flagged: bool,
    /// 本次对话激活过的技能名（去重）
    used_skills: HashSet<String>,
}

impl ThreadBuffer {
    fn new() -> Self {
        Self {
            report_id: Uuid::new_v4(),
            messages: Vec::new(),
            last_activity: Instant::now(),
            model_id: None,
            dlp_flagged: false,
            used_skills: HashSet::new(),
        }
    }

    fn touch(&mut self) {
        self.last_activity = Instant::now();
    }

    fn is_idle(&self, timeout: Duration) -> bool {
        self.last_activity.elapsed() > timeout
    }
}

/// 对话追踪器。
///
/// 线程安全，通过 `Arc<ConversationTracker>` 在 Tauri 命令和 Channel 之间共享。
pub const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(10 * 60);
pub const IDLE_FLUSH_INTERVAL: Duration = Duration::from_secs(60);

pub struct ConversationTracker {
    buffers: Mutex<HashMap<String, ThreadBuffer>>,
    scope_id: String,
    backend_user_id: Arc<std::sync::RwLock<Option<Uuid>>>,
    pub idle_timeout: Duration,
}

impl ConversationTracker {
    pub fn new(scope_id: String) -> Self {
        Self {
            buffers: Mutex::new(HashMap::new()),
            scope_id,
            backend_user_id: Arc::new(std::sync::RwLock::new(None)),
            idle_timeout: DEFAULT_IDLE_TIMEOUT,
        }
    }

    pub fn with_backend_user_id_sink(
        mut self,
        backend_user_id: Arc<std::sync::RwLock<Option<Uuid>>>,
    ) -> Self {
        self.backend_user_id = backend_user_id;
        self
    }

    /// 记录用户消息。
    pub fn record_user_message(
        &self,
        thread_id: &str,
        content: &str,
        dlp_flagged: bool,
        attachments: &[ConversationAttachment],
    ) {
        let mut buffers = self.lock_buffers();
        let buf = buffers
            .entry(thread_id.to_string())
            .or_insert_with(ThreadBuffer::new);
        buf.touch();
        if dlp_flagged {
            buf.dlp_flagged = true;
        }
        buf.messages.push(ConversationMessage {
            role: "user".to_string(),
            content: content.to_string(),
            attachments: attachments.to_vec(),
            model_id: None,
            input_tokens: 0,
            output_tokens: 0,
        });
    }

    /// 记录本轮命中的技能名。
    pub fn record_activated_skills(&self, thread_id: &str, skill_names: &[String]) {
        if skill_names.is_empty() {
            return;
        }

        let mut buffers = self.lock_buffers();
        let buf = buffers
            .entry(thread_id.to_string())
            .or_insert_with(ThreadBuffer::new);
        buf.touch();
        buf.used_skills
            .extend(skill_names.iter().filter(|name| !name.is_empty()).cloned());
    }

    /// 记录 assistant 回复（Token 数由后续 `update_last_assistant_tokens` 回填）。
    pub fn record_assistant_message(
        &self,
        thread_id: &str,
        content: &str,
        model_id: Option<&str>,
        input_tokens: i32,
        output_tokens: i32,
    ) {
        let mut buffers = self.lock_buffers();
        let buf = buffers
            .entry(thread_id.to_string())
            .or_insert_with(ThreadBuffer::new);
        buf.touch();
        if let Some(m) = model_id {
            buf.model_id = Some(m.to_string());
        }
        buf.messages.push(ConversationMessage {
            role: "assistant".to_string(),
            content: content.to_string(),
            attachments: Vec::new(),
            model_id: model_id.map(|s| s.to_string()),
            input_tokens,
            output_tokens,
        });
    }

    /// 回填最后一条 assistant 消息的 Token 信息。
    ///
    /// 由 `TauriChannel::send_status(TurnCost)` 调用：
    /// `respond()` 先记录消息（Token 为 0），`TurnCost` 事件到达后再更新。
    /// `model_id` 为空字符串时不更新模型字段。
    pub fn update_last_assistant_tokens(
        &self,
        thread_id: &str,
        model_id: &str,
        input_tokens: i32,
        output_tokens: i32,
    ) {
        let mut buffers = self.lock_buffers();
        let Some(buf) = buffers.get_mut(thread_id) else {
            return;
        };
        if let Some(msg) = buf
            .messages
            .iter_mut()
            .rev()
            .find(|m| m.role == "assistant")
        {
            if !model_id.is_empty() {
                msg.model_id = Some(model_id.to_string());
                buf.model_id = Some(model_id.to_string());
            }
            msg.input_tokens = input_tokens;
            msg.output_tokens = output_tokens;
        }
    }

    /// 显式结束对话并上报（切换 thread 或关闭应用时调用）。
    pub fn finish_thread(&self, thread_id: &str, reporter: &DataReporter) {
        let report = self
            .lock_buffers()
            .remove(thread_id)
            .and_then(|buf| self.build_report(thread_id, buf));
        if let Some(r) = report {
            reporter.enqueue(r);
        }
    }

    /// 检查并 flush 所有超时的对话。
    ///
    /// 建议在后台定期调用（如每 5 分钟一次）。
    pub fn flush_idle_threads(&self, reporter: &DataReporter) {
        let mut buffers = self.lock_buffers();
        let idle_keys: Vec<String> = buffers
            .iter()
            .filter(|(_, buf)| buf.is_idle(self.idle_timeout))
            .map(|(k, _)| k.clone())
            .collect();

        for key in idle_keys {
            if let Some(buf) = buffers.remove(&key) {
                if let Some(report) = self.build_report(&key, buf) {
                    reporter.enqueue(report);
                }
            }
        }
    }

    /// 将 ThreadBuffer 转换为 ClientReport，消息为空时返回 None。
    fn build_report(&self, thread_id: &str, buf: ThreadBuffer) -> Option<ClientReport> {
        if buf.messages.is_empty() {
            return None;
        }

        let mut used_skills: Vec<String> = buf.used_skills.into_iter().collect();
        used_skills.sort();
        let report_user_id = self
            .backend_user_id
            .read()
            .ok()
            .and_then(|user_id| *user_id)
            .map(|user_id: Uuid| user_id.to_string())
            .unwrap_or_else(|| self.scope_id.clone());

        Some(ClientReport::Conversation {
            client_conversation_id: format!("{}-{}-{}", self.scope_id, thread_id, buf.report_id),
            user_id: report_user_id,
            topic: None,
            model_id: buf.model_id,
            dlp_flagged: Some(buf.dlp_flagged),
            dlp_details: None,
            used_skills,
            messages: buf.messages,
        })
    }

    /// 获取 buffers 锁，自动从 poison 中恢复。
    fn lock_buffers(&self) -> std::sync::MutexGuard<'_, HashMap<String, ThreadBuffer>> {
        match self.buffers.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_reporter::{ClientReport, DataReporter};

    fn make_tracker() -> ConversationTracker {
        ConversationTracker::new("user-test".to_string())
    }

    fn make_reporter() -> DataReporter {
        DataReporter::new(String::new(), String::new())
    }

    #[test]
    fn test_default_idle_timeout_is_ten_minutes() {
        let tracker = make_tracker();

        assert_eq!(tracker.idle_timeout, DEFAULT_IDLE_TIMEOUT);
    }

    #[test]
    fn test_record_and_finish_thread() {
        let tracker = make_tracker();
        let reporter = make_reporter();

        tracker.record_user_message("t1", "hello", false, &[]);
        tracker.record_assistant_message("t1", "hi", Some("gpt-4o"), 10, 5);
        tracker.finish_thread("t1", &reporter);

        assert_eq!(reporter.queue_len(), 1);
    }

    #[test]
    fn test_empty_thread_not_reported() {
        let tracker = make_tracker();
        let reporter = make_reporter();

        tracker.finish_thread("t-empty", &reporter);
        assert_eq!(reporter.queue_len(), 0);
    }

    #[test]
    fn test_dlp_flagged_propagates() {
        let tracker = make_tracker();
        let reporter = make_reporter();

        tracker.record_user_message("t2", "secret content", true, &[]);
        tracker.record_assistant_message("t2", "ok", None, 5, 3);
        tracker.finish_thread("t2", &reporter);

        assert_eq!(reporter.queue_len(), 1);
        // 验证 DLP 标记被正确传递
        let reports = reporter.drain_for_test();
        if let ClientReport::Conversation { dlp_flagged, .. } = &reports[0] {
            assert_eq!(*dlp_flagged, Some(true));
        } else {
            panic!("Expected Conversation report");
        }
    }

    #[test]
    fn test_idle_flush() {
        let mut tracker = ConversationTracker::new("user-test".to_string());
        tracker.idle_timeout = Duration::from_millis(1); // 极短超时用于测试
        let reporter = make_reporter();

        tracker.record_user_message("t3", "hello", false, &[]);
        tracker.record_assistant_message("t3", "hi", None, 5, 3);

        std::thread::sleep(Duration::from_millis(5));
        tracker.flush_idle_threads(&reporter);

        assert_eq!(reporter.queue_len(), 1);
    }

    #[test]
    fn test_conversation_id_format() {
        let tracker = make_tracker();
        let reporter = make_reporter();

        tracker.record_user_message("thread-abc", "hi", false, &[]);
        tracker.record_assistant_message("thread-abc", "hello", None, 3, 2);
        tracker.finish_thread("thread-abc", &reporter);

        let reports = reporter.drain_for_test();
        if let ClientReport::Conversation {
            client_conversation_id,
            ..
        } = &reports[0]
        {
            assert!(client_conversation_id.contains("user-test"));
            assert!(client_conversation_id.contains("thread-abc"));
        } else {
            panic!("Expected Conversation report");
        }
    }

    #[test]
    fn test_same_thread_multiple_reports_have_distinct_ids() {
        let tracker = make_tracker();
        let reporter = make_reporter();

        tracker.record_user_message("thread-repeat", "first", false, &[]);
        tracker.record_assistant_message("thread-repeat", "reply-1", None, 3, 2);
        tracker.finish_thread("thread-repeat", &reporter);

        let first_report_id = match &reporter.drain_for_test()[0] {
            ClientReport::Conversation {
                client_conversation_id,
                ..
            } => client_conversation_id.clone(),
            _ => panic!("Expected Conversation report"),
        };

        tracker.record_user_message("thread-repeat", "second", false, &[]);
        tracker.record_assistant_message("thread-repeat", "reply-2", None, 4, 3);
        tracker.finish_thread("thread-repeat", &reporter);

        let second_report_id = match &reporter.drain_for_test()[0] {
            ClientReport::Conversation {
                client_conversation_id,
                ..
            } => client_conversation_id.clone(),
            _ => panic!("Expected Conversation report"),
        };

        assert_ne!(first_report_id, second_report_id);
    }

    #[test]
    fn test_report_uses_backend_user_id_when_available() {
        let backend_user_id = std::sync::Arc::new(std::sync::RwLock::new(Some(
            Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
        )));
        let tracker = ConversationTracker::new("scope-test".to_string())
            .with_backend_user_id_sink(std::sync::Arc::clone(&backend_user_id));
        let reporter = make_reporter();

        tracker.record_user_message("thread-backend", "hello", false, &[]);
        tracker.record_assistant_message("thread-backend", "hi", None, 3, 2);
        tracker.finish_thread("thread-backend", &reporter);

        let reports = reporter.drain_for_test();
        if let ClientReport::Conversation {
            user_id,
            client_conversation_id,
            ..
        } = &reports[0]
        {
            assert_eq!(user_id, "550e8400-e29b-41d4-a716-446655440000");
            assert!(client_conversation_id.contains("scope-test"));
        } else {
            panic!("Expected Conversation report");
        }
    }

    #[test]
    fn test_report_falls_back_to_scope_id_when_backend_user_missing() {
        let backend_user_id = std::sync::Arc::new(std::sync::RwLock::new(None));
        let tracker = ConversationTracker::new("scope-fallback".to_string())
            .with_backend_user_id_sink(std::sync::Arc::clone(&backend_user_id));
        let reporter = make_reporter();

        tracker.record_user_message("thread-fallback", "hello", false, &[]);
        tracker.record_assistant_message("thread-fallback", "hi", None, 3, 2);
        tracker.finish_thread("thread-fallback", &reporter);

        let reports = reporter.drain_for_test();
        if let ClientReport::Conversation { user_id, .. } = &reports[0] {
            assert_eq!(user_id, "scope-fallback");
        } else {
            panic!("Expected Conversation report");
        }
    }

    #[test]
    fn test_report_falls_back_to_scope_id_when_backend_user_lock_poisoned() {
        let backend_user_id = std::sync::Arc::new(std::sync::RwLock::new(Some(Uuid::nil())));
        let poisoned_backend_user_id = std::sync::Arc::clone(&backend_user_id);
        let _ = std::panic::catch_unwind(move || {
            let _guard = poisoned_backend_user_id
                .write()
                .expect("write lock should succeed");
            panic!("poison backend user id lock");
        });

        let tracker = ConversationTracker::new("scope-read-failure".to_string())
            .with_backend_user_id_sink(backend_user_id);
        let reporter = make_reporter();

        tracker.record_user_message("thread-read-failure", "hello", false, &[]);
        tracker.record_assistant_message("thread-read-failure", "hi", None, 3, 2);
        tracker.finish_thread("thread-read-failure", &reporter);

        let reports = reporter.drain_for_test();
        if let ClientReport::Conversation { user_id, .. } = &reports[0] {
            assert_eq!(user_id, "scope-read-failure");
        } else {
            panic!("Expected Conversation report");
        }
    }

    #[test]
    fn test_activated_skills_are_deduplicated_in_report() {
        let tracker = make_tracker();
        let reporter = make_reporter();

        tracker.record_user_message("thread-skill", "use some skills", false, &[]);
        tracker.record_activated_skills(
            "thread-skill",
            &[
                "code-review-expert".to_string(),
                "code-review-expert".to_string(),
                "code-simplifier".to_string(),
            ],
        );
        tracker.finish_thread("thread-skill", &reporter);

        let reports = reporter.drain_for_test();
        if let ClientReport::Conversation { used_skills, .. } = &reports[0] {
            assert_eq!(
                used_skills,
                &vec!["code-review-expert".to_string(), "code-simplifier".to_string()]
            );
        } else {
            panic!("Expected Conversation report");
        }
    }
}
