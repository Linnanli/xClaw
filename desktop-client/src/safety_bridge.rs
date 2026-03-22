//! 安全桥接模块 — 统一 SafetyLayer 与 DLP 格式保留脱敏。
//!
//! `SafetyBridge` 是 IronClaw `SafetyLayer` 和桌面端 DLP 模块的统一入口，
//! 采用链式处理架构：
//!
//! ```text
//! 用户输入 ──► SafetyLayer.scan_inbound_for_secrets()
//!                │
//!                ├─ 检测到密钥/令牌 → 故障安全：拒绝发送
//!                │
//!                └─ 无密钥 → DlpSanitizer.sanitize()
//!                               │
//!                               ├─ 检测到 PII → 格式保留脱敏（330***618）
//!                               │
//!                               └─ 无 PII → 原样通过
//!
//! 工具输出 ──► SafetyLayer.sanitize_tool_output()
//!                │
//!                └─ 截断 / 注入检测 / 密钥清理 → 安全输出
//! ```
//!
//! # 设计原则
//!
//! 1. **故障安全（Fail-Safe）**：SafetyLayer 扫描失败时拒绝操作，不降级
//! 2. **链式处理**：SafetyLayer（密钥/注入）→ DLP（PII 格式保留脱敏）
//! 3. **审计追踪**：所有安全事件通过 `DataReporter` 上报
//! 4. **零 IronClaw 修改**：仅使用 SafetyLayer 公开 API

use std::sync::Arc;

use ironclaw::safety::SafetyLayer;
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument, warn};

use crate::data_reporter::{ClientReport, DataReporter};
use crate::dlp::sanitizer::{DlpSanitizer, SanitizationConfig, SanitizationResult, SanitizationStats};
use crate::dlp::DlpDetector;

/// SafetyBridge 扫描结果。
///
/// 统一了 SafetyLayer 和 DLP 的扫描结果，
/// 前端只需关心这一个类型。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeScanResult {
    /// 是否检测到敏感信息（PII 或密钥）。
    pub had_sensitive_data: bool,
    /// 脱敏后的内容（如果被阻止则为空字符串）。
    pub sanitized_content: String,
    /// 是否被阻止（密钥泄露或不可脱敏的敏感信息）。
    pub was_blocked: bool,
    /// 阻止原因（仅当 `was_blocked == true`）。
    pub block_reason: Option<String>,
    /// 脱敏统计。
    pub stats: BridgeStats,
}

/// SafetyBridge 统计信息。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BridgeStats {
    /// SafetyLayer 是否检测到密钥。
    pub secret_detected: bool,
    /// DLP 检测到的 PII 匹配数。
    pub pii_matches: usize,
    /// DLP 脱敏处理数。
    pub redacted_count: usize,
    /// DLP 阻止数。
    pub blocked_count: usize,
    /// DLP 警告数。
    pub warned_count: usize,
}

impl From<&SanitizationStats> for BridgeStats {
    fn from(stats: &SanitizationStats) -> Self {
        Self {
            secret_detected: false,
            pii_matches: stats.total_matches,
            redacted_count: stats.redacted_count,
            blocked_count: stats.blocked_count,
            warned_count: stats.warned_count,
        }
    }
}

/// 累计统计（线程安全，用于运行时监控）。
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct BridgeCumulativeStats {
    pub total_scans: u64,
    pub secret_blocks: u64,
    pub pii_detections: u64,
    pub pii_redactions: u64,
    pub pii_blocks: u64,
    pub clean_passes: u64,
}

/// 安全桥接器 — 统一安全扫描入口。
///
/// 持有 `SafetyLayer`（来自 IronClaw AppComponents）和 `DlpSanitizer`，
/// 提供链式安全扫描：密钥检测 → PII 格式保留脱敏。
pub struct SafetyBridge {
    /// IronClaw 安全层（密钥检测、注入防护、策略执行）。
    safety: Arc<SafetyLayer>,
    /// DLP 脱敏器（PII 格式保留脱敏）。
    /// 使用 RwLock 支持运行时热更新规则（sync_dlp_rules_from_admin）。
    sanitizer: std::sync::RwLock<DlpSanitizer>,
    /// 数据上报器（审计事件上报到 Admin Backend）。
    reporter: Option<Arc<DataReporter>>,
    /// 累计统计。
    cumulative: std::sync::Mutex<BridgeCumulativeStats>,
}

impl SafetyBridge {
    /// 创建 SafetyBridge。
    ///
    /// # 参数
    ///
    /// - `safety`: IronClaw SafetyLayer（从 AppComponents 获取）
    /// - `sanitization_config`: DLP 脱敏配置（可选，默认使用内置配置）
    /// - `reporter`: 数据上报器（可选，用于审计事件上报）
    pub fn new(
        safety: Arc<SafetyLayer>,
        sanitization_config: Option<SanitizationConfig>,
        reporter: Option<Arc<DataReporter>>,
    ) -> Self {
        let detector = DlpDetector::new();
        let config = sanitization_config.unwrap_or_default();
        let sanitizer = DlpSanitizer::new(detector, config);

        Self {
            safety,
            sanitizer: std::sync::RwLock::new(sanitizer),
            reporter,
            cumulative: std::sync::Mutex::new(BridgeCumulativeStats::default()),
        }
    }

    /// 热更新 DLP 规则。
    ///
    /// 用新的 `LeakPattern` 列表替换当前 DlpDetector 中的自定义规则，
    /// 内置规则（中文手机号、身份证等）保留。
    pub fn reload_patterns(&self, patterns: Vec<ironclaw_safety::LeakPattern>) {
        let config = {
            let r = self.sanitizer.read().unwrap_or_else(|p| p.into_inner());
            r.config().clone()
        };
        let mut detector = DlpDetector::new(); // 重新加载内置规则
        detector.add_patterns(patterns);
        let new_sanitizer = DlpSanitizer::new(detector, config);
        let mut w = self.sanitizer.write().unwrap_or_else(|p| p.into_inner());
        *w = new_sanitizer;
        tracing::info!("DLP patterns reloaded");
    }

    /// 扫描用户输入。
    ///
    /// 链式处理：
    /// 1. SafetyLayer 密钥扫描 → 检测到密钥则**故障安全**拒绝
    /// 2. DLP PII 扫描 → 格式保留脱敏（如 `330***618`）
    ///
    /// # 安全
    ///
    /// - 密钥检测失败时拒绝操作（Fail-Safe），不降级
    /// - 不在日志中记录原始内容
    #[instrument(skip(self, content), fields(content_len = content.len()))]
    pub fn scan_user_input(&self, content: &str) -> BridgeScanResult {
        self.increment_total_scans();

        // ── Step 1: SafetyLayer 密钥扫描 ──────────────────────────
        if let Some(warning) = self.safety.scan_inbound_for_secrets(content) {
            warn!("Secret detected in user input, blocking message");
            self.increment_secret_blocks();
            self.report_secret_block(&warning);

            return BridgeScanResult {
                had_sensitive_data: true,
                sanitized_content: String::new(),
                was_blocked: true,
                block_reason: Some(warning),
                stats: BridgeStats {
                    secret_detected: true,
                    ..Default::default()
                },
            };
        }

        // ── Step 2: DLP PII 格式保留脱敏 ──────────────────────────
        let dlp_result = self.sanitizer.read().unwrap_or_else(|p| p.into_inner()).sanitize(content);
        self.update_pii_stats(&dlp_result);

        if dlp_result.had_sensitive_data {
            debug!(
                total_matches = dlp_result.sanitization_stats.total_matches,
                was_blocked = dlp_result.was_blocked,
                "PII detected in user input"
            );
            self.report_pii_event(&dlp_result);
        } else {
            self.increment_clean_passes();
        }

        BridgeScanResult {
            had_sensitive_data: dlp_result.had_sensitive_data,
            sanitized_content: dlp_result.sanitized_content,
            was_blocked: dlp_result.was_blocked,
            block_reason: dlp_result.block_reason,
            stats: BridgeStats::from(&dlp_result.sanitization_stats),
        }
    }

    /// 扫描工具输出。
    ///
    /// 委托给 SafetyLayer 处理（截断、注入检测、密钥清理）。
    /// DLP PII 脱敏不应用于工具输出（工具输出发给 LLM，不发给用户）。
    #[instrument(skip(self, output), fields(output_len = output.len()))]
    pub fn scan_tool_output(&self, tool_name: &str, output: &str) -> ironclaw::safety::SanitizedOutput {
        debug!(tool_name = tool_name, "Scanning tool output");
        self.safety.sanitize_tool_output(tool_name, output)
    }

    /// 扫描出站请求体。
    ///
    /// 链式处理：SafetyLayer 密钥扫描 → DLP PII 脱敏。
    /// 用于在发送 HTTP 请求前清理请求体中的敏感信息。
    #[instrument(skip(self, body), fields(body_len = body.len()))]
    pub fn scan_outbound(&self, body: &str) -> BridgeScanResult {
        self.increment_total_scans();

        // 密钥扫描
        if let Some(warning) = self.safety.scan_inbound_for_secrets(body) {
            warn!("Secret detected in outbound request, blocking");
            self.increment_secret_blocks();
            self.report_secret_block(&warning);

            return BridgeScanResult {
                had_sensitive_data: true,
                sanitized_content: String::new(),
                was_blocked: true,
                block_reason: Some(warning),
                stats: BridgeStats {
                    secret_detected: true,
                    ..Default::default()
                },
            };
        }

        // PII 脱敏
        let dlp_result = self.sanitizer.read().unwrap_or_else(|p| p.into_inner()).sanitize(body);
        self.update_pii_stats(&dlp_result);

        if dlp_result.had_sensitive_data {
            self.report_pii_event(&dlp_result);
        } else {
            self.increment_clean_passes();
        }

        BridgeScanResult {
            had_sensitive_data: dlp_result.had_sensitive_data,
            sanitized_content: dlp_result.sanitized_content,
            was_blocked: dlp_result.was_blocked,
            block_reason: dlp_result.block_reason,
            stats: BridgeStats::from(&dlp_result.sanitization_stats),
        }
    }

    /// 为存储脱敏内容。
    ///
    /// 与 `scan_user_input` 相同的链式处理，但阻止时返回 Err。
    #[instrument(skip(self, content), fields(content_len = content.len()))]
    pub fn sanitize_for_storage(&self, content: &str) -> Result<String, String> {
        let result = self.scan_user_input(content);
        if result.was_blocked {
            Err(result.block_reason.unwrap_or_else(|| "Content blocked".to_string()))
        } else {
            Ok(result.sanitized_content)
        }
    }

    /// 获取累计统计信息。
    pub fn cumulative_stats(&self) -> BridgeCumulativeStats {
        let guard = self.cumulative.lock().unwrap_or_else(|p| p.into_inner());
        BridgeCumulativeStats {
            total_scans: guard.total_scans,
            secret_blocks: guard.secret_blocks,
            pii_detections: guard.pii_detections,
            pii_redactions: guard.pii_redactions,
            pii_blocks: guard.pii_blocks,
            clean_passes: guard.clean_passes,
        }
    }

    /// 获取 DLP 脱敏配置引用（克隆）。
    pub fn sanitization_config(&self) -> SanitizationConfig {
        self.sanitizer.read().unwrap_or_else(|p| p.into_inner()).config().clone()
    }

    // ── 内部统计方法 ──────────────────────────────────────────────

    fn increment_total_scans(&self) {
        if let Ok(mut s) = self.cumulative.lock() {
            s.total_scans += 1;
        }
    }

    fn increment_secret_blocks(&self) {
        if let Ok(mut s) = self.cumulative.lock() {
            s.secret_blocks += 1;
        }
    }

    fn increment_clean_passes(&self) {
        if let Ok(mut s) = self.cumulative.lock() {
            s.clean_passes += 1;
        }
    }

    fn update_pii_stats(&self, result: &SanitizationResult) {
        if let Ok(mut s) = self.cumulative.lock() {
            if result.had_sensitive_data {
                s.pii_detections += 1;
            }
            s.pii_redactions += result.sanitization_stats.redacted_count as u64;
            s.pii_blocks += result.sanitization_stats.blocked_count as u64;
        }
    }

    // ── 审计上报 ──────────────────────────────────────────────────

    fn report_secret_block(&self, _warning: &str) {
        if let Some(ref reporter) = self.reporter {
            reporter.enqueue(ClientReport::DlpEvent {
                timestamp: iso_timestamp_now(),
                had_sensitive_data: true,
                was_blocked: true,
                // 不上报原始内容，只上报事件类型
                rule_matches: vec!["secret_leak_blocked".to_string()],
            });
            debug!("Secret block event enqueued for reporting");
        }
    }

    fn report_pii_event(&self, result: &SanitizationResult) {
        if let Some(ref reporter) = self.reporter {
            // 只上报模式名称，不上报原始数据
            let rule_matches: Vec<String> = result
                .sanitization_stats
                .severity_stats
                .keys()
                .cloned()
                .collect();

            reporter.enqueue(ClientReport::DlpEvent {
                timestamp: iso_timestamp_now(),
                had_sensitive_data: result.had_sensitive_data,
                was_blocked: result.was_blocked,
                rule_matches,
            });
            debug!("PII event enqueued for reporting");
        }
    }
}

/// 生成 ISO 8601 时间戳（UTC 近似）。
///
/// 不依赖 chrono，使用 `SystemTime` + `UNIX_EPOCH` 计算。
fn iso_timestamp_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // 简单 UTC 时间戳格式化（精确到秒）
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    // 从 1970-01-01 计算日期
    let (year, month, day) = days_to_ymd(days);

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hours, minutes, seconds
    )
}

/// 将 Unix epoch 天数转换为 (year, month, day)。
fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    // 简化的公历计算
    let mut y = 1970;
    let mut remaining = days;

    loop {
        let days_in_year = if is_leap_year(y) { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        y += 1;
    }

    let month_days = if is_leap_year(y) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut m = 0;
    for (i, &md) in month_days.iter().enumerate() {
        if remaining < md {
            m = i;
            break;
        }
        remaining -= md;
    }

    (y, (m + 1) as u64, remaining + 1)
}

fn is_leap_year(y: u64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

#[cfg(test)]
mod internal_tests {
    use super::*;

    #[test]
    fn test_iso_timestamp_format() {
        let ts = iso_timestamp_now();
        assert!(ts.ends_with('Z'), "Should end with Z: {}", ts);
        assert!(ts.contains('T'), "Should contain T: {}", ts);
        assert_eq!(ts.len(), 20, "Should be 20 chars: {}", ts);

        let year: u32 = ts[..4].parse().unwrap();
        assert!(year >= 2024 && year <= 2100, "Year: {}", year);
    }

    #[test]
    fn test_days_to_ymd_epoch() {
        let (y, m, d) = days_to_ymd(0);
        assert_eq!((y, m, d), (1970, 1, 1));
    }

    #[test]
    fn test_days_to_ymd_known_date() {
        // 2024-01-01 = 19723 days since epoch
        let (y, m, d) = days_to_ymd(19723);
        assert_eq!(y, 2024);
        assert_eq!(m, 1);
        assert_eq!(d, 1);
    }

    #[test]
    fn test_is_leap_year() {
        assert!(is_leap_year(2000));
        assert!(is_leap_year(2024));
        assert!(!is_leap_year(1900));
        assert!(!is_leap_year(2023));
    }
}
