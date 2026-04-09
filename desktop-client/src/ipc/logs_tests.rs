//! `ipc/logs.rs` 单元测试。
//!
//! 测试策略：直接构造 `LogBroadcaster` + `AppState` 的最小替代，
//! 调用内部辅助逻辑，不依赖 Tauri State 注入。

use std::sync::Arc;

use ironclaw::channels::web::log_layer::{LogBroadcaster, LogEntry};

use crate::ipc::logs::LogEntryDto;

// ── 测试辅助 ─────────────────────────────────────────────────────

fn make_entry(level: &str, target: &str, message: &str) -> LogEntry {
    LogEntry {
        level: level.to_string(),
        target: target.to_string(),
        message: message.to_string(),
        timestamp: "2026-04-05T12:00:00.000Z".to_string(),
    }
}

/// 构造一个填充了若干条目的 broadcaster。
fn broadcaster_with(entries: &[(&str, &str, &str)]) -> Arc<LogBroadcaster> {
    let b = Arc::new(LogBroadcaster::new());
    for &(level, target, msg) in entries {
        b.send(make_entry(level, target, msg));
    }
    b
}

// ── entries_after_offset 逻辑（提取为可测试的纯函数）────────────

fn entries_after_offset_pure(
    broadcaster: &LogBroadcaster,
    offset: usize,
    limit: usize,
) -> Vec<LogEntryDto> {
    let all = broadcaster.recent_entries();
    let sliced: Vec<_> = all.into_iter().skip(offset).collect();
    let start = sliced.len().saturating_sub(limit);
    sliced.into_iter().skip(start).map(Into::into).collect()
}

fn search_pure(
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
    let start = filtered.len().saturating_sub(limit);
    filtered.into_iter().skip(start).map(Into::into).collect()
}

fn filter_pure(
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
    let start = filtered.len().saturating_sub(limit);
    filtered.into_iter().skip(start).map(Into::into).collect()
}

// ── 正常路径测试 ─────────────────────────────────────────────────

#[test]
fn test_get_logs_returns_all_entries() {
    let b = broadcaster_with(&[
        ("INFO", "mod_a", "msg 1"),
        ("WARN", "mod_b", "msg 2"),
        ("ERROR", "mod_c", "msg 3"),
    ]);
    let result = entries_after_offset_pure(&b, 0, 100);
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].message, "msg 1");
    assert_eq!(result[2].message, "msg 3");
}

#[test]
fn test_get_logs_respects_limit() {
    let b = broadcaster_with(&[
        ("INFO", "m", "a"),
        ("INFO", "m", "b"),
        ("INFO", "m", "c"),
        ("INFO", "m", "d"),
        ("INFO", "m", "e"),
    ]);
    // limit=3 应返回最新的 3 条（c, d, e）
    let result = entries_after_offset_pure(&b, 0, 3);
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].message, "c");
    assert_eq!(result[2].message, "e");
}

#[test]
fn test_get_logs_empty_broadcaster() {
    let b = Arc::new(LogBroadcaster::new());
    let result = entries_after_offset_pure(&b, 0, 100);
    assert!(result.is_empty());
}

// ── 清空偏移测试 ─────────────────────────────────────────────────

#[test]
fn test_clear_hides_existing_entries() {
    let b = broadcaster_with(&[("INFO", "m", "old 1"), ("INFO", "m", "old 2")]);
    // 模拟 ic_clear_logs：offset = 当前条目数
    let offset = b.recent_entries().len();
    assert_eq!(offset, 2);

    // 清空后查询应为空
    let result = entries_after_offset_pure(&b, offset, 100);
    assert!(result.is_empty());
}

#[test]
fn test_new_entries_visible_after_clear() {
    let b = broadcaster_with(&[("INFO", "m", "old 1"), ("INFO", "m", "old 2")]);
    let offset = b.recent_entries().len(); // 清空点

    // 清空后写入新日志
    b.send(make_entry("INFO", "m", "new 1"));
    b.send(make_entry("INFO", "m", "new 2"));

    let result = entries_after_offset_pure(&b, offset, 100);
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].message, "new 1");
    assert_eq!(result[1].message, "new 2");
}

#[test]
fn test_clear_offset_zero_returns_all() {
    let b = broadcaster_with(&[("INFO", "m", "a"), ("INFO", "m", "b")]);
    // offset=0 等同于未清空
    let result = entries_after_offset_pure(&b, 0, 100);
    assert_eq!(result.len(), 2);
}

// ── 搜索测试 ─────────────────────────────────────────────────────

#[test]
fn test_search_matches_message() {
    let b = broadcaster_with(&[
        ("INFO", "mod", "engine started"),
        ("INFO", "mod", "DLP rules synced"),
        ("INFO", "mod", "engine ready"),
    ]);
    let result = search_pure(&b, 0, "engine", 100);
    assert_eq!(result.len(), 2);
    assert!(result.iter().all(|e| e.message.contains("engine")));
}

#[test]
fn test_search_matches_module() {
    let b = broadcaster_with(&[
        ("INFO", "ironclaw::config", "loading"),
        ("INFO", "desktop_client::engine", "starting"),
        ("INFO", "ironclaw::agent", "ready"),
    ]);
    let result = search_pure(&b, 0, "ironclaw", 100);
    assert_eq!(result.len(), 2);
}

#[test]
fn test_search_case_insensitive() {
    let b = broadcaster_with(&[("INFO", "mod", "Engine Started")]);
    let result = search_pure(&b, 0, "engine", 100);
    assert_eq!(result.len(), 1);
}

#[test]
fn test_search_no_match_returns_empty() {
    let b = broadcaster_with(&[("INFO", "mod", "hello world")]);
    let result = search_pure(&b, 0, "xyz_not_found", 100);
    assert!(result.is_empty());
}

#[test]
fn test_search_respects_offset() {
    let b = broadcaster_with(&[("INFO", "mod", "engine old"), ("INFO", "mod", "engine new")]);
    // 清空后只有第 2 条可见
    let result = search_pure(&b, 1, "engine", 100);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].message, "engine new");
}

// ── 过滤测试 ─────────────────────────────────────────────────────

#[test]
fn test_filter_by_level() {
    let b = broadcaster_with(&[
        ("INFO", "mod", "info msg"),
        ("WARN", "mod", "warn msg"),
        ("ERROR", "mod", "error msg"),
    ]);
    let result = filter_pure(&b, 0, "warn", "", 100);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].level, "WARN");
}

#[test]
fn test_filter_level_case_insensitive() {
    let b = broadcaster_with(&[("ERROR", "mod", "boom")]);
    // 前端传来的可能是小写
    let result = filter_pure(&b, 0, "error", "", 100);
    assert_eq!(result.len(), 1);
}

#[test]
fn test_filter_by_module() {
    let b = broadcaster_with(&[
        ("INFO", "ironclaw::agent", "agent msg"),
        ("INFO", "desktop_client::ipc", "ipc msg"),
        ("INFO", "ironclaw::config", "config msg"),
    ]);
    let result = filter_pure(&b, 0, "", "ironclaw", 100);
    assert_eq!(result.len(), 2);
    assert!(result.iter().all(|e| e.module.contains("ironclaw")));
}

#[test]
fn test_filter_level_and_module_combined() {
    let b = broadcaster_with(&[
        ("ERROR", "ironclaw::agent", "agent error"),
        ("WARN", "ironclaw::agent", "agent warn"),
        ("ERROR", "desktop_client", "client error"),
    ]);
    let result = filter_pure(&b, 0, "error", "ironclaw", 100);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].message, "agent error");
}

#[test]
fn test_filter_empty_strings_returns_all() {
    let b = broadcaster_with(&[("INFO", "mod_a", "a"), ("WARN", "mod_b", "b")]);
    // level="" module="" 不过滤
    let result = filter_pure(&b, 0, "", "", 100);
    assert_eq!(result.len(), 2);
}

#[test]
fn test_filter_respects_offset() {
    let b = broadcaster_with(&[("ERROR", "mod", "old error"), ("ERROR", "mod", "new error")]);
    let result = filter_pure(&b, 1, "error", "", 100);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].message, "new error");
}

// ── 导出测试 ─────────────────────────────────────────────────────

#[test]
fn test_export_produces_valid_json() {
    let b = broadcaster_with(&[("INFO", "mod", "hello"), ("ERROR", "mod", "world")]);
    let entries: Vec<LogEntryDto> = b.recent_entries().into_iter().map(Into::into).collect();
    let json = serde_json::to_string_pretty(&entries).expect("should serialize");

    // 验证是合法 JSON 数组
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&json).expect("should parse");
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0]["message"], "hello");
    assert_eq!(parsed[1]["level"], "ERROR");
}

#[test]
fn test_export_empty_is_empty_array() {
    let b = Arc::new(LogBroadcaster::new());
    let entries: Vec<LogEntryDto> = b.recent_entries().into_iter().map(Into::into).collect();
    let json = serde_json::to_string_pretty(&entries).expect("should serialize");
    assert_eq!(json.trim(), "[]");
}
// ── LogEntryDto From 转换测试 ─────────────────────────────────────

#[test]
fn test_log_entry_dto_field_mapping() {
    let entry = make_entry("DEBUG", "some::module", "test message");
    let dto: LogEntryDto = entry.into();
    assert_eq!(dto.level, "DEBUG");
    assert_eq!(dto.module, "some::module"); // target → module
    assert_eq!(dto.message, "test message");
    assert!(!dto.timestamp.is_empty());
}

// ── ic_export_logs 序列化测试 ─────────────────────────────────────

#[test]
fn test_export_logs_json_is_valid_and_contains_all_entries() {
    let b = broadcaster_with(&[("INFO", "mod", "hello"), ("ERROR", "mod", "world")]);
    let entries: Vec<LogEntryDto> = b.recent_entries().into_iter().map(Into::into).collect();
    let json = serde_json::to_string_pretty(&entries).expect("should serialize");

    let parsed: Vec<serde_json::Value> = serde_json::from_str(&json).expect("should parse");
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0]["message"], "hello");
    assert_eq!(parsed[1]["level"], "ERROR");
}

#[test]
fn test_export_logs_respects_clear_offset() {
    let b = broadcaster_with(&[("INFO", "mod", "old"), ("INFO", "mod", "new")]);
    let offset = 1; // 清空后只有第 2 条可见
    let entries: Vec<LogEntryDto> = b
        .recent_entries()
        .into_iter()
        .skip(offset)
        .map(Into::into)
        .collect();
    let json = serde_json::to_string_pretty(&entries).expect("should serialize");
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&json).expect("should parse");
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0]["message"], "new");
}

#[test]
fn test_export_logs_writes_to_temp_dir() {
    let b = broadcaster_with(&[("INFO", "mod", "test entry")]);
    let entries: Vec<LogEntryDto> = b.recent_entries().into_iter().map(Into::into).collect();
    let json = serde_json::to_string_pretty(&entries).expect("should serialize");

    // 模拟写文件逻辑
    let dir = std::env::temp_dir();
    let path = dir.join("ironclaw-logs-test.json");
    std::fs::write(&path, &json).expect("should write");

    let read_back = std::fs::read_to_string(&path).expect("should read");
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&read_back).expect("should parse");
    assert_eq!(parsed[0]["message"], "test entry");

    // 清理
    let _ = std::fs::remove_file(&path);
}
