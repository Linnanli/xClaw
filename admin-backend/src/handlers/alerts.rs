//! 告警与通知系统 Handler
//!
//! 端点：
//! - GET    /api/alert-rules              — 告警规则列表
//! - POST   /api/alert-rules              — 创建告警规则
//! - PUT    /api/alert-rules/{id}         — 更新告警规则
//! - DELETE /api/alert-rules/{id}         — 删除告警规则
//! - GET    /api/alerts                   — 告警事件列表
//! - GET    /api/alerts/stats             — 告警统计
//! - GET    /api/alerts/unhandled-count   — 未处理告警数量
//! - PUT    /api/alerts/{id}/status       — 更新告警事件状态
//! - POST   /api/alerts/trigger           — 手动触发告警（运维/测试用）

use crate::error::{Error, Result};
use crate::models::{
    AlertEventQuery, AlertTrigger, CreateAlertRuleRequest, UpdateAlertEventStatusRequest,
    UpdateAlertRuleRequest,
};
use crate::routes::write_audit_log;
use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde_json::json;
use uuid::Uuid;

// ============================================================================
// 告警规则 CRUD
// ============================================================================

/// GET /api/alert-rules
pub async fn get_alert_rules(State(state): State<AppState>) -> Result<Json<serde_json::Value>> {
    let client = state
        .db_pool
        .get()
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let rows = client
        .query(
            "SELECT id, name, description, event_type, condition, severity,
                    notify_channels, silence_minutes, enabled, created_at, updated_at
             FROM alert_rules ORDER BY created_at DESC",
            &[],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let rules: Vec<_> = rows.iter().map(row_to_alert_rule).collect();
    Ok(Json(json!({ "rules": rules, "total": rules.len() })))
}

/// POST /api/alert-rules
pub async fn create_alert_rule(
    State(state): State<AppState>,
    Json(payload): Json<CreateAlertRuleRequest>,
) -> Result<Json<serde_json::Value>> {
    validate_alert_rule_fields(&payload.event_type, &payload.severity)?;

    let client = state
        .db_pool
        .get()
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let channels_json = serde_json::to_value(&payload.notify_channels)
        .map_err(|e| Error::Validation(e.to_string()))?;

    let row = client
        .query_one(
            "INSERT INTO alert_rules (name, description, event_type, condition, severity,
                                      notify_channels, silence_minutes, enabled)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             RETURNING id, created_at",
            &[
                &payload.name,
                &payload.description,
                &payload.event_type,
                &payload.condition,
                &payload.severity,
                &channels_json,
                &payload.silence_minutes,
                &payload.enabled,
            ],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let id: Uuid = row.get(0);

    write_audit_log(
        &client,
        Uuid::nil(),
        "create_alert_rule",
        &format!("创建告警规则: {}", payload.name),
    )
    .await;

    Ok(Json(json!({
        "id": id,
        "message": "告警规则已创建",
    })))
}

/// PUT /api/alert-rules/{id}
pub async fn update_alert_rule(
    State(state): State<AppState>,
    Path(rule_id): Path<Uuid>,
    Json(payload): Json<UpdateAlertRuleRequest>,
) -> Result<Json<serde_json::Value>> {
    if let Some(ref et) = payload.event_type {
        validate_event_type(et)?;
    }
    if let Some(ref sev) = payload.severity {
        validate_severity(sev)?;
    }

    let client = state
        .db_pool
        .get()
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    // 验证规则存在
    let existing = client
        .query_opt("SELECT name FROM alert_rules WHERE id = $1", &[&rule_id])
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .ok_or_else(|| Error::NotFound("告警规则不存在".into()))?;
    let old_name: String = existing.get(0);

    let (sql, params) = build_alert_rule_update_sql(rule_id, &payload);
    let param_refs: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = params
        .iter()
        .map(|p| &**p as &(dyn tokio_postgres::types::ToSql + Sync))
        .collect();

    client
        .execute(&sql, &param_refs)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    write_audit_log(
        &client,
        Uuid::nil(),
        "update_alert_rule",
        &format!("更新告警规则: {}", old_name),
    )
    .await;

    Ok(Json(json!({ "message": "告警规则已更新" })))
}

/// DELETE /api/alert-rules/{id}
pub async fn delete_alert_rule(
    State(state): State<AppState>,
    Path(rule_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    let client = state
        .db_pool
        .get()
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let row = client
        .query_opt("SELECT name FROM alert_rules WHERE id = $1", &[&rule_id])
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .ok_or_else(|| Error::NotFound("告警规则不存在".into()))?;
    let name: String = row.get(0);

    client
        .execute("DELETE FROM alert_rules WHERE id = $1", &[&rule_id])
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    write_audit_log(
        &client,
        Uuid::nil(),
        "delete_alert_rule",
        &format!("删除告警规则: {}", name),
    )
    .await;

    Ok(Json(json!({ "message": "告警规则已删除" })))
}

// ============================================================================
// 告警事件
// ============================================================================

/// GET /api/alerts
pub async fn get_alert_events(
    State(state): State<AppState>,
    Query(params): Query<AlertEventQuery>,
) -> Result<Json<serde_json::Value>> {
    let client = state
        .db_pool
        .get()
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let page = params.page.unwrap_or(1).max(1);
    let page_size = params.page_size.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1) * page_size;

    let (where_clause, query_params) = build_alert_events_filter(&params);
    let param_refs: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = query_params
        .iter()
        .map(|p| &**p as &(dyn tokio_postgres::types::ToSql + Sync))
        .collect();

    // 总数查询
    let count_sql = format!("SELECT COUNT(*) FROM alert_events {}", where_clause);
    let total: i64 = client
        .query_one(&count_sql, &param_refs)
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .get(0);

    // 分页数据查询
    let data_sql = format!(
        "SELECT id, rule_id, rule_name, event_type, severity, trigger_detail,
                event_data, status, resolved_note, resolved_at, created_at
         FROM alert_events {}
         ORDER BY created_at DESC
         LIMIT {} OFFSET {}",
        where_clause, page_size, offset
    );
    let rows = client
        .query(&data_sql, &param_refs)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let events: Vec<_> = rows.iter().map(row_to_alert_event).collect();

    Ok(Json(json!({
        "data": events,
        "total": total,
        "page": page,
        "page_size": page_size,
    })))
}

/// GET /api/alerts/stats
pub async fn get_alert_stats(State(state): State<AppState>) -> Result<Json<serde_json::Value>> {
    let client = state
        .db_pool
        .get()
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let row = client
        .query_one(
            "SELECT
                COUNT(*) FILTER (WHERE status = 'pending') as pending,
                COUNT(*) FILTER (WHERE status = 'in_progress') as in_progress,
                COUNT(*) FILTER (WHERE created_at >= CURRENT_DATE) as today,
                COUNT(*) FILTER (WHERE status = 'closed') as closed
             FROM alert_events",
            &[],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    Ok(Json(json!({
        "pending": row.get::<_, i64>(0),
        "in_progress": row.get::<_, i64>(1),
        "today": row.get::<_, i64>(2),
        "closed": row.get::<_, i64>(3),
    })))
}

/// GET /api/alerts/unhandled-count — 未处理告警数量（需求 15.8）
pub async fn get_unhandled_alert_count(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>> {
    let client = state
        .db_pool
        .get()
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let row = client
        .query_one(
            "SELECT COUNT(*) FROM alert_events WHERE status = 'pending'",
            &[],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    Ok(Json(json!({ "count": row.get::<_, i64>(0) })))
}

/// PUT /api/alerts/{id}/status
pub async fn update_alert_event_status(
    State(state): State<AppState>,
    Path(event_id): Path<Uuid>,
    Json(payload): Json<UpdateAlertEventStatusRequest>,
) -> Result<Json<serde_json::Value>> {
    validate_alert_status(&payload.status)?;

    let client = state
        .db_pool
        .get()
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let existing = client
        .query_opt(
            "SELECT rule_name FROM alert_events WHERE id = $1",
            &[&event_id],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .ok_or_else(|| Error::NotFound("告警事件不存在".into()))?;
    let rule_name: String = existing.get(0);

    let resolved_at = if payload.status == "closed" || payload.status == "acknowledged" {
        Some(chrono::Utc::now())
    } else {
        None
    };

    client
        .execute(
            "UPDATE alert_events
             SET status = $1, resolved_note = $2, resolved_at = COALESCE($3, resolved_at)
             WHERE id = $4",
            &[&payload.status, &payload.note, &resolved_at, &event_id],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    write_audit_log(
        &client,
        Uuid::nil(),
        "update_alert_status",
        &format!("更新告警事件状态: {} → {}", rule_name, payload.status),
    )
    .await;

    Ok(Json(json!({ "message": "告警状态已更新" })))
}

// ============================================================================
// trigger_alert — 核心告警触发函数（供其他模块调用）
// ============================================================================

/// 触发告警：查询匹配的已启用规则 → 检查静默期 → 生成告警事件 → 记录通知日志
///
/// 返回生成的告警事件数量。如果没有匹配规则，返回 0。
pub async fn trigger_alert(
    pool: &deadpool_postgres::Pool,
    trigger: &AlertTrigger,
) -> std::result::Result<usize, String> {
    let client = pool.get().await.map_err(|e| e.to_string())?;

    let rules = client
        .query(
            "SELECT id, name, severity, notify_channels, silence_minutes
             FROM alert_rules
             WHERE event_type = $1 AND enabled = true",
            &[&trigger.event_type],
        )
        .await
        .map_err(|e| e.to_string())?;

    let mut created_count = 0usize;

    for rule in &rules {
        let rule_id: Uuid = rule.get(0);
        let rule_name: String = rule.get(1);
        let rule_severity: String = rule.get(2);
        let channels: serde_json::Value = rule.get(3);
        let silence_minutes: i32 = rule.get(4);

        if is_in_silence_period(&client, rule_id, silence_minutes).await? {
            continue;
        }

        let severity = if trigger.severity.is_empty() {
            &rule_severity
        } else {
            &trigger.severity
        };

        let event_id = insert_alert_event(
            &client,
            rule_id,
            &rule_name,
            &trigger.event_type,
            severity,
            &trigger.detail,
            &trigger.event_data,
        )
        .await?;

        insert_notification_logs(&client, event_id, &channels).await;

        // 异步发送通知，不阻塞告警触发流程
        let pool_clone = pool.clone();
        let rule_name_clone = rule_name.clone();
        let severity_clone = severity.to_string();
        let detail_clone = trigger.detail.clone();
        tokio::spawn(async move {
            send_alert_notifications(
                &pool_clone,
                event_id,
                &rule_name_clone,
                &severity_clone,
                &detail_clone,
            )
            .await;
        });

        created_count += 1;
    }

    Ok(created_count)
}

/// 插入告警事件记录，返回事件 ID
async fn insert_alert_event(
    client: &deadpool_postgres::Object,
    rule_id: Uuid,
    rule_name: &str,
    event_type: &str,
    severity: &str,
    detail: &str,
    event_data: &Option<serde_json::Value>,
) -> std::result::Result<Uuid, String> {
    let row = client
        .query_one(
            "INSERT INTO alert_events (rule_id, rule_name, event_type, severity, trigger_detail, event_data)
             VALUES ($1, $2, $3, $4, $5, $6)
             RETURNING id",
            &[&rule_id, &rule_name, &event_type, &severity, &detail, event_data],
        )
        .await
        .map_err(|e| e.to_string())?;
    Ok(row.get(0))
}

/// 为告警事件的每个通知渠道创建通知日志
async fn insert_notification_logs(
    client: &deadpool_postgres::Object,
    event_id: Uuid,
    channels: &serde_json::Value,
) {
    let channel_arr = match channels.as_array() {
        Some(arr) => arr,
        None => return,
    };
    for ch in channel_arr {
        if let Some(channel_name) = ch.as_str() {
            let _ = client
                .execute(
                    "INSERT INTO notification_logs (alert_event_id, channel, status)
                     VALUES ($1, $2, 'pending')",
                    &[&event_id, &channel_name],
                )
                .await;
        }
    }
}

/// POST /api/alerts/trigger — 手动触发告警（运维/测试用）
pub async fn manual_trigger_alert(
    State(state): State<AppState>,
    Json(payload): Json<AlertTrigger>,
) -> Result<Json<serde_json::Value>> {
    validate_event_type(&payload.event_type)?;

    let count = trigger_alert(&state.db_pool, &payload)
        .await
        .map_err(|e| Error::Internal(e))?;

    Ok(Json(json!({
        "message": format!("已触发 {} 条告警", count),
        "count": count,
    })))
}

// ============================================================================
// 内部辅助函数
// ============================================================================

const VALID_EVENT_TYPES: &[&str] = &[
    "dlp_violation",
    "quota_exceeded",
    "model_error",
    "abnormal_login",
    "approval_timeout",
];

const VALID_SEVERITIES: &[&str] = &["low", "medium", "high", "critical"];

const VALID_ALERT_STATUSES: &[&str] = &["pending", "acknowledged", "in_progress", "closed"];

fn validate_event_type(event_type: &str) -> Result<()> {
    validate_in_list(event_type, VALID_EVENT_TYPES, "事件类型")
}

fn validate_severity(severity: &str) -> Result<()> {
    validate_in_list(severity, VALID_SEVERITIES, "严重级别")
}

fn validate_alert_rule_fields(event_type: &str, severity: &str) -> Result<()> {
    validate_event_type(event_type)?;
    validate_severity(severity)
}

fn validate_alert_status(status: &str) -> Result<()> {
    validate_in_list(status, VALID_ALERT_STATUSES, "告警状态")
}

/// 通用列表验证：检查值是否在允许列表中
fn validate_in_list(value: &str, valid: &[&str], label: &str) -> Result<()> {
    if !valid.contains(&value) {
        return Err(Error::Validation(format!(
            "无效的{}: {}，有效值: {:?}",
            label, value, valid
        )));
    }
    Ok(())
}

/// 检查规则是否在静默期内
async fn is_in_silence_period(
    client: &deadpool_postgres::Object,
    rule_id: Uuid,
    silence_minutes: i32,
) -> std::result::Result<bool, String> {
    let row = client
        .query_one(
            "SELECT COUNT(*) FROM alert_events
             WHERE rule_id = $1
               AND created_at > NOW() - ($2 || ' minutes')::INTERVAL",
            &[&rule_id, &silence_minutes.to_string()],
        )
        .await
        .map_err(|e| e.to_string())?;

    let count: i64 = row.get(0);
    Ok(count > 0)
}

/// 行映射：alert_rules 表行 → JSON
fn row_to_alert_rule(row: &tokio_postgres::Row) -> serde_json::Value {
    json!({
        "id": row.get::<_, Uuid>(0),
        "name": row.get::<_, String>(1),
        "description": row.get::<_, Option<String>>(2),
        "event_type": row.get::<_, String>(3),
        "condition": row.get::<_, serde_json::Value>(4),
        "severity": row.get::<_, String>(5),
        "notify_channels": row.get::<_, serde_json::Value>(6),
        "silence_minutes": row.get::<_, i32>(7),
        "enabled": row.get::<_, bool>(8),
        "created_at": row.get::<_, chrono::DateTime<chrono::Utc>>(9),
        "updated_at": row.get::<_, chrono::DateTime<chrono::Utc>>(10),
    })
}

/// 行映射：alert_events 表行 → JSON
fn row_to_alert_event(row: &tokio_postgres::Row) -> serde_json::Value {
    json!({
        "id": row.get::<_, Uuid>(0),
        "rule_id": row.get::<_, Option<Uuid>>(1),
        "rule_name": row.get::<_, String>(2),
        "event_type": row.get::<_, String>(3),
        "severity": row.get::<_, String>(4),
        "trigger_detail": row.get::<_, String>(5),
        "event_data": row.get::<_, Option<serde_json::Value>>(6),
        "status": row.get::<_, String>(7),
        "resolved_note": row.get::<_, Option<String>>(8),
        "resolved_at": row.get::<_, Option<chrono::DateTime<chrono::Utc>>>(9),
        "created_at": row.get::<_, chrono::DateTime<chrono::Utc>>(10),
    })
}

/// 构建告警事件筛选 WHERE 子句
fn build_alert_events_filter(
    params: &AlertEventQuery,
) -> (
    String,
    Vec<Box<dyn tokio_postgres::types::ToSql + Sync + Send>>,
) {
    let mut conditions: Vec<String> = Vec::new();
    let mut query_params: Vec<Box<dyn tokio_postgres::types::ToSql + Sync + Send>> = Vec::new();
    let mut idx = 1;

    if let Some(ref severity) = params.severity {
        let trimmed = severity.trim();
        if !trimmed.is_empty() {
            conditions.push(format!("severity = ${}", idx));
            query_params.push(Box::new(trimmed.to_string()));
            idx += 1;
        }
    }

    if let Some(ref status) = params.status {
        let trimmed = status.trim();
        if !trimmed.is_empty() {
            conditions.push(format!("status = ${}", idx));
            query_params.push(Box::new(trimmed.to_string()));
            idx += 1;
        }
    }

    if let Some(ref search) = params.search {
        let trimmed = search.trim();
        if !trimmed.is_empty() {
            conditions.push(format!(
                "(rule_name ILIKE ${} OR trigger_detail ILIKE ${})",
                idx, idx
            ));
            query_params.push(Box::new(format!("%{}%", trimmed)));
        }
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    (where_clause, query_params)
}

/// 构建告警规则更新 SQL（动态字段）
fn build_alert_rule_update_sql(
    rule_id: Uuid,
    payload: &UpdateAlertRuleRequest,
) -> (
    String,
    Vec<Box<dyn tokio_postgres::types::ToSql + Sync + Send>>,
) {
    let mut sets: Vec<String> = Vec::new();
    let mut params: Vec<Box<dyn tokio_postgres::types::ToSql + Sync + Send>> = Vec::new();
    let mut idx = 1;

    if let Some(ref name) = payload.name {
        sets.push(format!("name = ${}", idx));
        params.push(Box::new(name.clone()));
        idx += 1;
    }
    if let Some(ref desc) = payload.description {
        sets.push(format!("description = ${}", idx));
        params.push(Box::new(desc.clone()));
        idx += 1;
    }
    if let Some(ref et) = payload.event_type {
        sets.push(format!("event_type = ${}", idx));
        params.push(Box::new(et.clone()));
        idx += 1;
    }
    if let Some(ref cond) = payload.condition {
        sets.push(format!("condition = ${}", idx));
        params.push(Box::new(cond.clone()));
        idx += 1;
    }
    if let Some(ref sev) = payload.severity {
        sets.push(format!("severity = ${}", idx));
        params.push(Box::new(sev.clone()));
        idx += 1;
    }
    if let Some(ref channels) = payload.notify_channels {
        let json_val = serde_json::to_value(channels).unwrap_or_default();
        sets.push(format!("notify_channels = ${}", idx));
        params.push(Box::new(json_val));
        idx += 1;
    }
    if let Some(silence) = payload.silence_minutes {
        sets.push(format!("silence_minutes = ${}", idx));
        params.push(Box::new(silence));
        idx += 1;
    }
    if let Some(enabled) = payload.enabled {
        sets.push(format!("enabled = ${}", idx));
        params.push(Box::new(enabled));
        idx += 1;
    }

    sets.push(format!("updated_at = NOW()"));

    let sql = format!(
        "UPDATE alert_rules SET {} WHERE id = ${}",
        sets.join(", "),
        idx
    );
    params.push(Box::new(rule_id));

    (sql, params)
}

// ============================================================================
// 通知发送服务
// ============================================================================

/// 通知渠道配置（从 system_settings 读取，key 前缀 `alert_`）
struct NotificationChannelConfig {
    wecom_webhook: Option<String>,
    dingtalk_webhook: Option<String>,
    feishu_webhook: Option<String>,
    email_smtp_host: Option<String>,
    email_smtp_port: Option<u16>,
    email_from: Option<String>,
    email_to: Option<String>,
}

impl NotificationChannelConfig {
    async fn load(client: &deadpool_postgres::Object) -> Self {
        let rows = client
            .query(
                "SELECT key, value FROM system_settings WHERE key LIKE 'alert_%'",
                &[],
            )
            .await
            .unwrap_or_default();

        let get = |key: &str| -> Option<String> {
            rows.iter()
                .find(|r| r.get::<_, String>(0) == key)
                .and_then(|r| {
                    r.get::<_, serde_json::Value>(1)
                        .as_str()
                        .map(|s| s.to_string())
                })
        };

        Self {
            wecom_webhook: get("alert_wecom_webhook"),
            dingtalk_webhook: get("alert_dingtalk_webhook"),
            feishu_webhook: get("alert_feishu_webhook"),
            email_smtp_host: get("alert_email_smtp_host"),
            email_smtp_port: get("alert_email_smtp_port").and_then(|s| s.parse().ok()),
            email_from: get("alert_email_from"),
            email_to: get("alert_email_to"),
        }
    }
}

/// 发送告警通知到所有配置的渠道，并更新 notification_logs 状态。
///
/// 在 trigger_alert 生成告警事件后异步调用，不阻塞主流程。
pub async fn send_alert_notifications(
    pool: &deadpool_postgres::Pool,
    event_id: Uuid,
    rule_name: &str,
    severity: &str,
    detail: &str,
) {
    let client = match pool.get().await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to get DB connection for notifications");
            return;
        }
    };

    let config = NotificationChannelConfig::load(&client).await;

    let logs = match client
        .query(
            "SELECT id, channel FROM notification_logs \
             WHERE alert_event_id = $1 AND status = 'pending'",
            &[&event_id],
        )
        .await
    {
        Ok(rows) => rows,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to query notification logs");
            return;
        }
    };

    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_default();

    let message = format!("[{}] {} — {}", severity.to_uppercase(), rule_name, detail);

    for log in &logs {
        let log_id: Uuid = log.get(0);
        let channel: String = log.get(1);

        let result = match channel.as_str() {
            "wecom" => {
                send_webhook(
                    &http,
                    config.wecom_webhook.as_deref(),
                    &build_wecom_payload(&message),
                )
                .await
            }
            "dingtalk" => {
                send_webhook(
                    &http,
                    config.dingtalk_webhook.as_deref(),
                    &build_dingtalk_payload(&message),
                )
                .await
            }
            "feishu" => {
                send_webhook(
                    &http,
                    config.feishu_webhook.as_deref(),
                    &build_feishu_payload(&message),
                )
                .await
            }
            "email" => send_email_notification(&config, &message).await,
            other => Err(format!("Unknown channel: {}", other)),
        };

        let (status, error) = match result {
            Ok(()) => ("success".to_string(), None::<String>),
            Err(e) => {
                tracing::warn!(channel = %channel, error = %e, "Notification send failed");
                ("failed".to_string(), Some(e))
            }
        };

        let _ = client
            .execute(
                "UPDATE notification_logs \
                 SET status = $1, last_error = $2, attempts = attempts + 1, updated_at = NOW() \
                 WHERE id = $3",
                &[&status, &error, &log_id],
            )
            .await;
    }
}

async fn send_webhook(
    http: &reqwest::Client,
    url: Option<&str>,
    payload: &serde_json::Value,
) -> std::result::Result<(), String> {
    let url = url.ok_or_else(|| "Webhook URL not configured".to_string())?;
    http.post(url)
        .json(payload)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn build_wecom_payload(message: &str) -> serde_json::Value {
    json!({ "msgtype": "text", "text": { "content": message } })
}

fn build_dingtalk_payload(message: &str) -> serde_json::Value {
    json!({ "msgtype": "text", "text": { "content": message } })
}

fn build_feishu_payload(message: &str) -> serde_json::Value {
    json!({ "msg_type": "text", "content": { "text": message } })
}

/// 邮件通知（当前实现：记录日志，实际 SMTP 发送留 TODO）
///
/// TODO: 引入 `lettre` crate 实现真实 SMTP 发送。
/// 政企场景通常优先使用企业微信/钉钉/飞书，邮件作为备选渠道。
async fn send_email_notification(
    config: &NotificationChannelConfig,
    message: &str,
) -> std::result::Result<(), String> {
    let smtp_host = config
        .email_smtp_host
        .as_deref()
        .ok_or_else(|| "Email SMTP host not configured".to_string())?;
    let smtp_port = config.email_smtp_port.unwrap_or(465);
    let from = config
        .email_from
        .as_deref()
        .ok_or_else(|| "Email sender not configured".to_string())?;
    let to = config
        .email_to
        .as_deref()
        .ok_or_else(|| "Email recipient not configured".to_string())?;

    // TODO: 使用 lettre crate 实现真实 SMTP 发送
    // 示例：SmtpTransport::relay(smtp_host)?.port(smtp_port).build()
    //        .send(Message::builder().from(from).to(to).body(message))
    tracing::info!(
        smtp_host = %smtp_host,
        smtp_port = %smtp_port,
        from = %from,
        to = %to,
        message = %message,
        "Email notification queued (SMTP not yet implemented)"
    );
    Ok(())
}
