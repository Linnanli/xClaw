//! DLP 桥接 Tauri Commands。
//!
//! 将前端的 DLP 命令桥接到新架构的 `SafetyBridge`。
//! 命令名称与旧 `commands.rs` 保持一致，前端零改动。
//!
//! # 命令映射
//!
//! | 前端调用 | 旧实现 | 新实现 |
//! |---------|--------|--------|
//! | `scan_user_input` | `DlpIntegration` | `SafetyBridge::scan_user_input` |
//! | `scan_outbound_request` | `DlpIntegration` | `SafetyBridge::scan_outbound` |
//! | `sanitize_for_storage` | `DlpIntegration` | `SafetyBridge::sanitize_for_storage` |
//! | `check_http_request` | `DlpIntegration` | `SafetyBridge::scan_outbound` |
//! | `get_dlp_config` | `DlpIntegration` | `SafetyBridge::sanitization_config` |
//! | `get_dlp_statistics` | `DlpIntegration` | `SafetyBridge::cumulative_stats` |
//! | `sync_dlp_rules_from_admin` | Admin API | No-op (admin sync 在引擎启动时完成) |

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::safety_bridge::BridgeScanResult;
use crate::state::AppState;

// ============================================================================
// 前端兼容类型
// ============================================================================

/// 前端期望的脱敏结果格式。
///
/// 字段名与旧 `commands.rs` 中的 `SanitizationResult` 保持一致，
/// 确保前端 `useDlpScan.ts` 无需修改。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DlpScanResponse {
    pub had_sensitive_data: bool,
    pub sanitized_content: String,
    pub was_blocked: bool,
    pub block_reason: Option<String>,
    pub sanitization_stats: DlpScanStats,
}

/// 前端期望的统计格式。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DlpScanStats {
    pub total_matches: usize,
    pub redacted_count: usize,
    pub blocked_count: usize,
    pub warned_count: usize,
}

impl From<BridgeScanResult> for DlpScanResponse {
    fn from(r: BridgeScanResult) -> Self {
        Self {
            had_sensitive_data: r.had_sensitive_data,
            sanitized_content: r.sanitized_content,
            was_blocked: r.was_blocked,
            block_reason: r.block_reason,
            sanitization_stats: DlpScanStats {
                total_matches: r.stats.pii_matches + if r.stats.secret_detected { 1 } else { 0 },
                redacted_count: r.stats.redacted_count,
                blocked_count: r.stats.blocked_count + if r.stats.secret_detected { 1 } else { 0 },
                warned_count: r.stats.warned_count,
            },
        }
    }
}

/// 前端期望的 DLP 统计信息格式。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DlpStatisticsResponse {
    pub total_scans: u64,
    pub sensitive_data_detected: u64,
    pub content_blocked: u64,
    pub content_sanitized: u64,
    pub http_requests_blocked: u64,
}

/// 前端期望的 DLP 配置格式。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DlpConfigResponse {
    pub enabled: bool,
    pub sanitization: DlpSanitizationConfig,
    pub real_time_monitoring: bool,
    pub audit_logging: bool,
    pub custom_patterns: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DlpSanitizationConfig {
    pub redaction_text: String,
    pub preserve_format: bool,
    pub partial_redaction: bool,
}

/// DLP 规则同步结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncDlpResult {
    pub success: bool,
    pub rules_synced: usize,
    pub message: String,
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// 扫描用户输入的敏感信息。
///
/// 链式处理：SafetyLayer 密钥检测 → DLP PII 格式保留脱敏。
/// 前端 `useDlpScan.ts` 的 `scanUserInput()` 调用此命令。
#[tauri::command]
pub async fn scan_user_input(
    state: State<'_, AppState>,
    content: String,
) -> Result<DlpScanResponse, String> {
    let result = state.safety_bridge.scan_user_input(&content);
    Ok(DlpScanResponse::from(result))
}

/// 扫描出站请求体。
///
/// 前端 `useDlpScan.ts` 的 `scanOutboundRequest()` 调用此命令。
#[tauri::command]
pub async fn scan_outbound_request(
    state: State<'_, AppState>,
    body: String,
) -> Result<DlpScanResponse, String> {
    let result = state.safety_bridge.scan_outbound(&body);
    Ok(DlpScanResponse::from(result))
}

/// 为存储脱敏内容。
///
/// 前端 `useDlpScan.ts` 的 `sanitizeForStorage()` 调用此命令。
#[tauri::command]
pub async fn sanitize_for_storage(
    state: State<'_, AppState>,
    content: String,
) -> Result<String, String> {
    state.safety_bridge.sanitize_for_storage(&content)
}

/// 检查 HTTP 请求是否包含敏感信息。
///
/// 前端 `useDlpScan.ts` 的 `checkHttpRequest()` 调用此命令。
/// 新架构下复用 `scan_outbound` 扫描请求体。
#[tauri::command]
pub async fn check_http_request(
    state: State<'_, AppState>,
    url: String,
    headers: Vec<(String, String)>,
    body: Option<Vec<u8>>,
) -> Result<(), String> {
    // 扫描 URL
    let url_result = state.safety_bridge.scan_outbound(&url);
    if url_result.was_blocked {
        return Err(format!("URL contains sensitive data: {}", 
            url_result.block_reason.unwrap_or_default()));
    }

    // 扫描 headers
    for (key, value) in &headers {
        let header_str = format!("{}: {}", key, value);
        let header_result = state.safety_bridge.scan_outbound(&header_str);
        if header_result.was_blocked {
            return Err(format!("Header '{}' contains sensitive data", key));
        }
    }

    // 扫描 body
    if let Some(body_bytes) = body {
        if let Ok(body_str) = String::from_utf8(body_bytes) {
            let body_result = state.safety_bridge.scan_outbound(&body_str);
            if body_result.was_blocked {
                return Err(format!("Request body contains sensitive data: {}",
                    body_result.block_reason.unwrap_or_default()));
            }
        }
    }

    Ok(())
}

/// 获取 DLP 配置。
///
/// 前端 `useDlpScan.ts` 的 `getDlpConfig()` 调用此命令。
/// 从 SafetyBridge 的脱敏配置构建前端期望的格式。
#[tauri::command]
pub async fn get_dlp_config(
    state: State<'_, AppState>,
) -> Result<DlpConfigResponse, String> {
    let config = state.safety_bridge.sanitization_config();

    Ok(DlpConfigResponse {
        enabled: config.enabled,
        sanitization: DlpSanitizationConfig {
            redaction_text: config.default_redaction.clone(),
            preserve_format: config.preserve_format,
            partial_redaction: config.preserve_format, // 格式保留即部分脱敏
        },
        real_time_monitoring: true,
        audit_logging: true,
        custom_patterns: vec![],
    })
}

/// 更新 DLP 配置。
///
/// 新架构下 DLP 配置由 Admin Backend 统一管理，
/// 客户端不支持本地修改。返回成功但不执行操作。
#[tauri::command]
pub async fn update_dlp_config(
    _config: serde_json::Value,
) -> Result<(), String> {
    tracing::warn!("update_dlp_config is no-op in embedded mode; config managed by admin");
    Ok(())
}

/// 获取 DLP 统计信息。
///
/// 前端 `useDlpScan.ts` 的 `getDlpStatistics()` 调用此命令。
#[tauri::command]
pub async fn get_dlp_statistics(
    state: State<'_, AppState>,
) -> Result<DlpStatisticsResponse, String> {
    let stats = state.safety_bridge.cumulative_stats();

    Ok(DlpStatisticsResponse {
        total_scans: stats.total_scans,
        sensitive_data_detected: stats.pii_detections + stats.secret_blocks,
        content_blocked: stats.secret_blocks + stats.pii_blocks,
        content_sanitized: stats.pii_redactions,
        http_requests_blocked: 0, // 新架构中 HTTP 拦截统计暂不单独追踪
    })
}

/// 从 Admin Backend 同步 DLP 规则。
///
/// 新架构下 DLP 规则在引擎启动时通过 `admin_sync` 模块同步，
/// 此命令保留为兼容前端调用，返回成功状态。
#[tauri::command]
pub async fn sync_dlp_rules_from_admin() -> Result<SyncDlpResult, String> {
    tracing::debug!("sync_dlp_rules_from_admin called (handled by admin_sync in embedded mode)");

    Ok(SyncDlpResult {
        success: true,
        rules_synced: 0,
        message: "DLP rules managed by embedded SafetyBridge".to_string(),
    })
}
