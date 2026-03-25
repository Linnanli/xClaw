//! 工具函数（从 ironclaw 主 crate 提取，避免循环依赖）

/// 截断字符串用于日志预览
pub fn truncate_for_preview(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}…", &s[..max_len])
    }
}

/// 最小化的 settings store trait（替代 ironclaw::db::Database 的 settings 子集）
#[async_trait::async_trait]
pub trait SettingsStore: Send + Sync {
    async fn get_setting(&self, user_id: &str, key: &str) -> Result<Option<serde_json::Value>, Box<dyn std::error::Error + Send + Sync>>;
    async fn set_setting(&self, user_id: &str, key: &str, value: &serde_json::Value) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}
