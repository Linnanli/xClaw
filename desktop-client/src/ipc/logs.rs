//! 日志查询 Tauri Commands。
//!
//! 从 `LogBroadcaster` 的内存环形缓冲区读取运行时日志，
//! 供 LogsTab 展示、搜索、过滤和导出。
//!
//! # 清空逻辑
//!
//! `LogBroadcaster` 不支持清空，通过 `AppState.log_clear_offset` 记录
//! 清空时的日志总数，后续查询跳过 offset 之前的条目，实现"视觉清空"。

use std::sync::atomic::{AtomicUsize, Ordering};

use ironclaw::channels::web::log_layer::{LogBroadcaster, LogEntry};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::state::EngineState;

/// 前端展示用的日志条目（与 `ironclaw::LogEntry` 字段对齐）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntryDto {
    pub timestamp: String,
    pub level: String,
    /// 模块路径（对应 ironclaw LogEntry.target）
    pub module: String,
    pub message: String,
}

impl From<ironclaw::channels::web::log_layer::LogEntry> for LogEntryDto {
    fn from(e: ironclaw::channels::web::log_layer::LogEntry) -> Self {
        Self {
            timestamp: e.timestamp,
            level: e.level,
            module: e.target,
            message: e.message,
        }
    }
}

/// 获取清空偏移后的日志条目（最多 `limit` 条，时间正序）。
fn entries_after_offset(state: &crate::state::AppState, limit: usize) -> Vec<LogEntryDto> {
    let offset = state.log_clear_offset.load(Ordering::Relaxed);
    entries_after_offset_from(&state.log_broadcaster, offset, limit)
}

/// 从已过滤的条目中取最后 `limit` 条并转换为 DTO。
pub(crate) fn take_last(entries: Vec<LogEntry>, limit: usize) -> Vec<LogEntryDto> {
    let start = entries.len().saturating_sub(limit);
    entries.into_iter().skip(start).map(Into::into).collect()
}

pub(crate) fn visible_limit(limit: Option<usize>) -> usize {
    limit.unwrap_or(100).min(1000)
}

pub(crate) fn entries_after_offset_from(
    broadcaster: &LogBroadcaster,
    offset: usize,
    limit: usize,
) -> Vec<LogEntryDto> {
    let sliced: Vec<_> = broadcaster
        .recent_entries()
        .into_iter()
        .skip(offset)
        .collect();
    take_last(sliced, limit)
}

pub(crate) fn search_entries(
    broadcaster: &LogBroadcaster,
    offset: usize,
    query: &str,
    limit: usize,
) -> Vec<LogEntryDto> {
    let q = query.to_lowercase();
    let filtered: Vec<_> = broadcaster
        .recent_entries()
        .into_iter()
        .skip(offset)
        .filter(|e| e.message.to_lowercase().contains(&q) || e.target.to_lowercase().contains(&q))
        .collect();
    take_last(filtered, limit)
}

pub(crate) fn filter_entries(
    broadcaster: &LogBroadcaster,
    offset: usize,
    level: &str,
    module: &str,
    limit: usize,
) -> Vec<LogEntryDto> {
    let level_filter = level.to_uppercase();
    let module_filter = module.to_lowercase();
    let filtered: Vec<_> = broadcaster
        .recent_entries()
        .into_iter()
        .skip(offset)
        .filter(|e| {
            let level_ok = level_filter.is_empty() || e.level.to_uppercase() == level_filter;
            let module_ok =
                module_filter.is_empty() || e.target.to_lowercase().contains(&module_filter);
            level_ok && module_ok
        })
        .collect();
    take_last(filtered, limit)
}

pub(crate) fn export_entries_json(
    broadcaster: &LogBroadcaster,
    offset: usize,
) -> Result<String, serde_json::Error> {
    let entries: Vec<LogEntryDto> = broadcaster
        .recent_entries()
        .into_iter()
        .skip(offset)
        .map(Into::into)
        .collect();
    serde_json::to_string_pretty(&entries)
}

pub(crate) fn clear_log_offset(broadcaster: &LogBroadcaster, offset: &AtomicUsize) {
    let current_len = broadcaster.recent_entries().len();
    offset.store(current_len, Ordering::Relaxed);
}

/// 获取最近日志（最多 `limit` 条，默认 100）。
#[tauri::command]
pub async fn ic_get_logs(
    state: State<'_, EngineState>,
    limit: Option<usize>,
) -> Result<Vec<LogEntryDto>, String> {
    let state = state.get()?;
    Ok(entries_after_offset(state, visible_limit(limit)))
}

/// 按关键词搜索日志（在 message 和 module 中匹配，大小写不敏感）。
#[tauri::command]
pub async fn ic_search_logs(
    state: State<'_, EngineState>,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<LogEntryDto>, String> {
    let state = state.get()?;
    let offset = state.log_clear_offset.load(Ordering::Relaxed);
    Ok(search_entries(
        &state.log_broadcaster,
        offset,
        &query,
        visible_limit(limit),
    ))
}

/// 按级别和模块过滤日志。
///
/// `level` 为空字符串时不过滤级别；`module` 为空字符串时不过滤模块。
#[tauri::command]
pub async fn ic_filter_logs(
    state: State<'_, EngineState>,
    level: String,
    module: String,
    limit: Option<usize>,
) -> Result<Vec<LogEntryDto>, String> {
    let state = state.get()?;
    let offset = state.log_clear_offset.load(Ordering::Relaxed);
    Ok(filter_entries(
        &state.log_broadcaster,
        offset,
        &level,
        &module,
        visible_limit(limit),
    ))
}

/// 导出日志为 JSON 字符串（供前端用 dialog + fs 插件保存到用户选择的路径）。
#[tauri::command]
pub async fn ic_export_logs(state: State<'_, EngineState>) -> Result<String, String> {
    let state = state.get()?;
    let offset = state.log_clear_offset.load(Ordering::Relaxed);
    export_entries_json(&state.log_broadcaster, offset)
        .map_err(|e| format!("Serialization failed: {}", e))
}

/// 清空日志（视觉清空：记录当前日志总数作为偏移，后续查询跳过此前条目）。
#[tauri::command]
pub async fn ic_clear_logs(state: State<'_, EngineState>) -> Result<(), String> {
    let state = state.get()?;
    clear_log_offset(&state.log_broadcaster, &state.log_clear_offset);
    tracing::debug!("Log buffer visually cleared");
    Ok(())
}
