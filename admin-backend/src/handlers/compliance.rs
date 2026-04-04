//! 数据分类分级与合规 Handler
//!
//! 端点：
//! - GET  /api/compliance/overview        — 合规概览（各级别 DLP 规则数 + 拦截统计）
//! - GET  /api/compliance/reports         — 合规报告列表
//! - POST /api/compliance/reports         — 生成合规报告
//! - GET  /api/compliance/retention       — 数据保留策略列表
//! - PUT  /api/compliance/retention/{level} — 更新保留策略

use crate::error::{Error, Result};
use crate::models::{GenerateReportRequest, UpdateRetentionPolicyRequest};
use crate::routes::write_audit_log;
use crate::AppState;
use axum::{
    extract::{Path, State},
    Json,
};
use serde_json::json;
use uuid::Uuid;

const VALID_LEVELS: &[&str] = &["public", "internal", "confidential", "top_secret"];

const LEVEL_LABELS: &[(&str, &str)] = &[
    ("public", "公开"),
    ("internal", "内部"),
    ("confidential", "机密"),
    ("top_secret", "绝密"),
];

/// GET /api/compliance/overview — 合规概览
pub async fn get_compliance_overview(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let levels: Vec<_> = LEVEL_LABELS.iter().map(|(key, label)| {
        json!({ "key": key, "label": label })
    }).collect();

    // 各级别 DLP 规则数
    let rule_rows = client.query(
        "SELECT classification_level, COUNT(*) as cnt
         FROM dlp_rules WHERE classification_level IS NOT NULL
         GROUP BY classification_level",
        &[],
    ).await.map_err(|e| Error::Database(e.to_string()))?;

    let mut rule_counts = serde_json::Map::new();
    for r in &rule_rows {
        let level: String = r.get(0);
        let count: i64 = r.get(1);
        rule_counts.insert(level, json!(count));
    }

    // 总 DLP 规则数（用于计算覆盖率）
    let total_rules: i64 = client.query_one(
        "SELECT COUNT(*) FROM dlp_rules WHERE enabled = true", &[],
    ).await.map_err(|e| Error::Database(e.to_string()))?.get(0);

    // 各级别拦截统计（从 audit_logs 中按 DLP 规则的 classification_level 聚合）
    // 简化实现：直接统计 dlp_block 审计日志数量
    let block_count: i64 = client.query_one(
        "SELECT COUNT(*) FROM audit_logs WHERE action = 'dlp_block' AND created_at >= CURRENT_DATE - INTERVAL '30 days'",
        &[],
    ).await.map_err(|e| Error::Database(e.to_string()))?.get(0);

    Ok(Json(json!({
        "levels": levels,
        "rule_counts": rule_counts,
        "total_rules": total_rules,
        "blocks_last_30d": block_count,
    })))
}

/// GET /api/compliance/reports — 合规报告列表
pub async fn get_compliance_reports(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let rows = client.query(
        "SELECT r.id, r.name, r.report_type, r.start_date, r.end_date, r.created_at, u.username, r.content
         FROM compliance_reports r
         LEFT JOIN users u ON u.id = r.generated_by
         ORDER BY r.created_at DESC
         LIMIT 50",
        &[],
    ).await.map_err(|e| Error::Database(e.to_string()))?;

    let reports: Vec<_> = rows.iter().map(|r| json!({
        "id": r.get::<_, Uuid>(0),
        "name": r.get::<_, String>(1),
        "report_type": r.get::<_, String>(2),
        "start_date": r.get::<_, chrono::NaiveDate>(3).to_string(),
        "end_date": r.get::<_, chrono::NaiveDate>(4).to_string(),
        "created_at": r.get::<_, chrono::DateTime<chrono::Utc>>(5),
        "generated_by": r.get::<_, Option<String>>(6),
        "content": r.get::<_, serde_json::Value>(7),
    })).collect();

    Ok(Json(json!({ "reports": reports, "total": reports.len() })))
}

/// POST /api/compliance/reports — 生成合规报告
pub async fn generate_compliance_report(
    State(state): State<AppState>,
    Json(payload): Json<GenerateReportRequest>,
) -> Result<Json<serde_json::Value>> {
    let start = chrono::NaiveDate::parse_from_str(&payload.start_date, "%Y-%m-%d")
        .map_err(|_| Error::Validation("start_date 格式无效，应为 YYYY-MM-DD".into()))?;
    let end = chrono::NaiveDate::parse_from_str(&payload.end_date, "%Y-%m-%d")
        .map_err(|_| Error::Validation("end_date 格式无效，应为 YYYY-MM-DD".into()))?;

    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let start_ts = start.and_hms_opt(0, 0, 0).expect("valid midnight").and_utc();
    let end_ts = end.succ_opt().expect("valid next day").and_hms_opt(0, 0, 0).expect("valid midnight").and_utc();

    let content = build_report_content(&client, &start_ts, &end_ts).await?;
    let report_type = payload.report_type.as_deref().unwrap_or("custom");

    let row = client.query_one(
        "INSERT INTO compliance_reports (name, report_type, start_date, end_date, content)
         VALUES ($1, $2, $3, $4, $5) RETURNING id, created_at",
        &[&payload.name, &report_type, &start, &end, &content],
    ).await.map_err(|e| Error::Database(e.to_string()))?;

    write_audit_log(&client, Uuid::nil(), "generate_compliance_report",
        &format!("生成合规报告: {} ({} ~ {})", payload.name, payload.start_date, payload.end_date),
    ).await;

    Ok(Json(json!({
        "id": row.get::<_, Uuid>(0),
        "created_at": row.get::<_, chrono::DateTime<chrono::Utc>>(1),
        "message": "合规报告已生成",
    })))
}

/// GET /api/compliance/retention — 数据保留策略列表
pub async fn get_retention_policies(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let rows = client.query(
        "SELECT classification_level, retention_days, updated_at FROM data_retention_policies ORDER BY classification_level",
        &[],
    ).await.map_err(|e| Error::Database(e.to_string()))?;

    let policies: Vec<_> = rows.iter().map(|r| {
        let level: String = r.get(0);
        let label = LEVEL_LABELS.iter().find(|(k, _)| *k == level).map(|(_, l)| *l).unwrap_or(&level);
        json!({
            "classification_level": level,
            "label": label,
            "retention_days": r.get::<_, i32>(1),
            "updated_at": r.get::<_, chrono::DateTime<chrono::Utc>>(2),
        })
    }).collect();

    Ok(Json(json!({ "policies": policies })))
}

/// PUT /api/compliance/retention/{level} — 更新保留策略
pub async fn update_retention_policy(
    State(state): State<AppState>,
    Path(level): Path<String>,
    Json(payload): Json<UpdateRetentionPolicyRequest>,
) -> Result<Json<serde_json::Value>> {
    if !VALID_LEVELS.contains(&level.as_str()) {
        return Err(Error::Validation(format!("无效的分级: {}", level)));
    }
    if payload.retention_days < 1 {
        return Err(Error::Validation("保留天数必须大于 0".into()));
    }

    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    client.execute(
        "UPDATE data_retention_policies SET retention_days = $1, updated_at = NOW() WHERE classification_level = $2",
        &[&payload.retention_days, &level],
    ).await.map_err(|e| Error::Database(e.to_string()))?;

    let label = LEVEL_LABELS.iter().find(|(k, _)| *k == level).map(|(_, l)| *l).unwrap_or(&level);
    write_audit_log(&client, Uuid::nil(), "update_retention_policy",
        &format!("更新{}级数据保留策略: {} 天", label, payload.retention_days),
    ).await;

    Ok(Json(json!({ "message": "保留策略已更新" })))
}

// ============================================================================
// 内部辅助函数
// ============================================================================

/// 构建合规报告内容（聚合指定时间范围内的安全事件、DLP 统计、策略变更）
async fn build_report_content(
    client: &deadpool_postgres::Object,
    start: &chrono::DateTime<chrono::Utc>,
    end: &chrono::DateTime<chrono::Utc>,
) -> Result<serde_json::Value> {
    // DLP 拦截统计
    let dlp_blocks: i64 = client.query_one(
        "SELECT COUNT(*) FROM audit_logs WHERE action = 'dlp_block' AND created_at >= $1 AND created_at < $2",
        &[start, end],
    ).await.map_err(|e| Error::Database(e.to_string()))?.get(0);

    // 策略变更数
    let policy_changes: i64 = client.query_one(
        "SELECT COUNT(*) FROM policy_change_records WHERE changed_at >= $1 AND changed_at < $2",
        &[start, end],
    ).await.map_err(|e| Error::Database(e.to_string()))?.get(0);

    // 告警事件数
    let alert_events: i64 = client.query_one(
        "SELECT COUNT(*) FROM alert_events WHERE created_at >= $1 AND created_at < $2",
        &[start, end],
    ).await.map(|r| r.get(0)).unwrap_or(0);

    // 审批工单数
    let approval_tickets: i64 = client.query_one(
        "SELECT COUNT(*) FROM approval_tickets WHERE created_at >= $1 AND created_at < $2",
        &[start, end],
    ).await.map(|r| r.get(0)).unwrap_or(0);

    Ok(json!({
        "dlp_blocks": dlp_blocks,
        "policy_changes": policy_changes,
        "alert_events": alert_events,
        "approval_tickets": approval_tickets,
    }))
}
