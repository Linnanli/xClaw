//! 费用配额管理 Handler
//!
//! 端点：
//! - POST /api/quota/check              — 预检（客户端 AI 请求前调用）
//! - POST /api/quota/report-usage       — 上报 usage 并计费
//! - GET  /api/quota/overview           — 费用概览
//! - GET  /api/quota/config             — 获取配额配置
//! - PUT  /api/quota/config             — 更新配额配置
//! - GET  /api/quota/department-ranking  — 部门消耗排行
//! - GET  /api/quota/model-ranking      — 模型消耗排行
//! - GET  /api/quota/usage-records      — 费用消耗明细（验收标准18#6）
//! - GET  /api/departments/{id}/quota-summary — 根部门配额摘要（验收标准15）

use crate::error::{Error, Result};
use crate::handlers::alerts;
use crate::models::AlertTrigger;
use crate::AppState;
use axum::{extract::{Path, Query, State}, Json};
use chrono::{Datelike, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

// ============================================================================
// 请求/响应类型
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct QuotaCheckRequest {
    pub user_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct UsageReportRequest {
    pub user_id: Uuid,
    pub model_id: String,
    pub input_tokens: i32,
    pub output_tokens: i32,
}

#[derive(Debug, Serialize)]
pub struct QuotaCheckResponse {
    pub allowed: bool,
    pub reason: Option<String>,
    pub daily_used_cents: i64,
    pub daily_limit_cents: Option<i64>,
    pub root_daily_used_cents: Option<i64>,
    pub root_daily_limit_cents: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateQuotaConfigRequest {
    pub monthly_budget_cents: Option<i64>,
    pub org_daily_limit_cents: Option<i64>,
}

// ============================================================================
// POST /api/quota/check — 多层级预检（验收标准14）
//
// 检查顺序：直属部门限额 → 根部门（公司级）限额
// 任一超额即拒绝。直属部门 == 根部门时只检查一次。
// ============================================================================

pub async fn quota_check(
    State(state): State<AppState>,
    Json(payload): Json<QuotaCheckRequest>,
) -> Result<Json<QuotaCheckResponse>> {
    let client = state.db_pool.get().await
        .map_err(|_| Error::Validation("配额服务暂时不可用，请稍后重试".to_string()))?;

    let today_start = today_start_utc();
    let dept_id = query_user_department(&client, payload.user_id).await?;

    // 无部门用户：无限额，直接放行
    let Some(did) = dept_id else {
        return Ok(Json(QuotaCheckResponse {
            allowed: true, reason: None,
            daily_used_cents: 0, daily_limit_cents: None,
            root_daily_used_cents: None, root_daily_limit_cents: None,
        }));
    };

    // 1. 直属部门检查
    let dept_used = query_dept_daily_usage(&client, did, &today_start).await?;
    let dept_limit = query_department_daily_limit(&client, did).await?;
    let dept_ok = dept_limit.map_or(true, |l| dept_used < l);

    // 2. 根部门检查（直属 == 根时跳过）
    let root_id = find_root_department(&client, did).await?;
    let is_root = root_id == did;
    let (root_used, root_limit) = if is_root {
        (None, None) // 直属就是根，不重复检查
    } else {
        let used = query_global_daily_usage(&client, &today_start).await?;
        let limit = query_department_daily_limit(&client, root_id).await?;
        (Some(used), limit)
    };
    let root_ok = root_limit.map_or(true, |l| root_used.unwrap_or(0) < l);

    // 判定结果
    let (allowed, reason) = if !dept_ok {
        (false, Some("当日费用已达部门限额，请联系管理员".to_string()))
    } else if !root_ok {
        (false, Some("当日费用已达公司级限额，请联系管理员".to_string()))
    } else {
        (true, None)
    };

    Ok(Json(QuotaCheckResponse {
        allowed, reason,
        daily_used_cents: dept_used,
        daily_limit_cents: dept_limit,
        root_daily_used_cents: root_used,
        root_daily_limit_cents: root_limit,
    }))
}

// ============================================================================
// POST /api/quota/report-usage — 上报 usage 并计费
// ============================================================================

pub async fn report_usage(
    State(state): State<AppState>,
    Json(payload): Json<UsageReportRequest>,
) -> Result<Json<serde_json::Value>> {
    let cost_cents = report_usage_internal(&state.db_pool, payload).await?;
    Ok(Json(json!({ "cost_cents": cost_cents })))
}

/// 内部费用上报逻辑，供 HTTP handler 和对话摄取后的异步触发共用。
///
/// 返回计算出的费用（分）。
pub async fn report_usage_internal(
    pool: &deadpool_postgres::Pool,
    payload: UsageReportRequest,
) -> Result<i32> {
    let client = pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let dept_id = query_user_department(&client, payload.user_id).await?;
    let (model_config_id, cost_cents) = calculate_cost(
        &client, &payload.model_id, payload.input_tokens, payload.output_tokens,
    ).await?;

    client.execute(
        "INSERT INTO usage_records (user_id, department_id, model_config_id, model_id, input_tokens, output_tokens, cost_cents)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
        &[&payload.user_id, &dept_id, &model_config_id, &payload.model_id,
          &payload.input_tokens, &payload.output_tokens, &cost_cents],
    ).await.map_err(|e| Error::Database(e.to_string()))?;

    // 异步检查费用预警（不阻塞响应，失败静默）
    if let Some(did) = dept_id {
        let pool = pool.clone();
        tokio::spawn(async move {
            let _ = check_quota_warning(&pool, did).await;
        });
    }

    Ok(cost_cents)
}

// ============================================================================
// GET /api/quota/overview — 费用概览
// ============================================================================

pub async fn quota_overview(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let today_start = today_start_utc();
    let month_start = month_start_utc();

    let (today_cost, today_tokens) = query_period_usage(&client, &today_start).await?;
    let (month_cost, month_tokens) = query_period_usage(&client, &month_start).await?;

    let budget_row = client.query_opt(
        "SELECT monthly_budget_cents FROM quota_configs WHERE scope = 'org' AND scope_id IS NULL",
        &[],
    ).await.map_err(|e| Error::Database(e.to_string()))?;

    let monthly_budget: Option<i64> = budget_row.map(|r| r.get(0));
    let budget_usage_pct = monthly_budget.map(|b| if b > 0 { (month_cost as f64 / b as f64 * 100.0) as i32 } else { 0 });

    let active_models: i64 = client.query_one(
        "SELECT COUNT(DISTINCT model_id) FROM usage_records WHERE created_at >= $1",
        &[&month_start],
    ).await.map_err(|e| Error::Database(e.to_string()))?.get(0);

    Ok(Json(json!({
        "today_cost_cents": today_cost,
        "today_tokens": today_tokens,
        "month_cost_cents": month_cost,
        "month_tokens": month_tokens,
        "monthly_budget_cents": monthly_budget,
        "budget_usage_pct": budget_usage_pct,
        "active_models": active_models,
    })))
}

// ============================================================================
// GET /api/quota/config — 获取配额配置
// ============================================================================

pub async fn get_quota_config(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let org_row = client.query_opt(
        "SELECT daily_limit_cents, monthly_budget_cents FROM quota_configs
         WHERE scope = 'org' AND scope_id IS NULL",
        &[],
    ).await.map_err(|e| Error::Database(e.to_string()))?;

    let (daily_limit, monthly_budget) = match org_row {
        Some(r) => (r.get::<_, Option<i32>>(0), r.get::<_, Option<i32>>(1)),
        None => (None, None),
    };

    Ok(Json(json!({
        "org_daily_limit_cents": daily_limit,
        "monthly_budget_cents": monthly_budget,
    })))
}

// ============================================================================
// PUT /api/quota/config — 更新配额配置
// ============================================================================

pub async fn update_quota_config(
    State(state): State<AppState>,
    Json(payload): Json<UpdateQuotaConfigRequest>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let now = Utc::now();
    client.execute(
        "INSERT INTO quota_configs (scope, scope_id, daily_limit_cents, monthly_budget_cents, updated_at)
         VALUES ('org', NULL, $1, $2, $3)
         ON CONFLICT (scope, scope_id) DO UPDATE SET
           daily_limit_cents = $1, monthly_budget_cents = $2, updated_at = $3",
        &[&payload.org_daily_limit_cents.map(|v| v as i32),
          &payload.monthly_budget_cents.map(|v| v as i32),
          &now],
    ).await.map_err(|e| Error::Database(e.to_string()))?;

    Ok(Json(json!({ "message": "配额配置已更新" })))
}

// ============================================================================
// GET /api/quota/department-ranking — 部门消耗排行
// ============================================================================

pub async fn department_ranking(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let month_start = month_start_utc();
    let rows = client.query(
        "SELECT d.name, COALESCE(SUM(ur.cost_cents), 0) as total_cost,
                COALESCE(SUM(ur.input_tokens + ur.output_tokens), 0) as total_tokens
         FROM usage_records ur
         JOIN departments d ON d.id = ur.department_id
         WHERE ur.created_at >= $1
         GROUP BY d.id, d.name
         ORDER BY total_cost DESC
         LIMIT 10",
        &[&month_start],
    ).await.map_err(|e| Error::Database(e.to_string()))?;

    let ranking: Vec<_> = rows.iter().map(|r| json!({
        "name": r.get::<_, String>(0),
        "cost_cents": r.get::<_, i64>(1),
        "tokens": r.get::<_, i64>(2),
    })).collect();

    Ok(Json(json!({ "ranking": ranking })))
}

// ============================================================================
// GET /api/quota/model-ranking — 模型消耗排行
// ============================================================================

pub async fn model_ranking(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let month_start = month_start_utc();
    let rows = client.query(
        "SELECT ur.model_id, COALESCE(SUM(ur.cost_cents), 0) as total_cost,
                COALESCE(SUM(ur.input_tokens + ur.output_tokens), 0) as total_tokens
         FROM usage_records ur
         WHERE ur.created_at >= $1
         GROUP BY ur.model_id
         ORDER BY total_cost DESC
         LIMIT 10",
        &[&month_start],
    ).await.map_err(|e| Error::Database(e.to_string()))?;

    let ranking: Vec<_> = rows.iter().map(|r| json!({
        "model_id": r.get::<_, String>(0),
        "cost_cents": r.get::<_, i64>(1),
        "tokens": r.get::<_, i64>(2),
    })).collect();

    Ok(Json(json!({ "ranking": ranking })))
}

// ============================================================================
// GET /api/quota/usage-records — 费用消耗明细（验收标准18#6）
//
// 支持筛选：时间范围、用户名、模型 ID、部门 ID
// 支持分页：page + page_size（默认 page=1, page_size=20）
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct UsageRecordsQuery {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub username: Option<String>,
    pub model_id: Option<String>,
    pub department_id: Option<Uuid>,
}

pub async fn usage_records(
    State(state): State<AppState>,
    Query(q): Query<UsageRecordsQuery>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let page = q.page.unwrap_or(1).max(1);
    let page_size = q.page_size.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1) * page_size;

    // 默认时间范围：今日
    let start = q.start_date
        .as_deref()
        .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .map(|d| d.and_hms_opt(0, 0, 0).expect("valid midnight").and_utc())
        .unwrap_or_else(today_start_utc);

    let end = q.end_date
        .as_deref()
        .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .map(|d| d.succ_opt().expect("valid next day").and_hms_opt(0, 0, 0).expect("valid midnight").and_utc());

    // 构建动态 WHERE 子句（复用项目中 build_departments_query 的 Box 模式）
    let mut conditions = vec!["ur.created_at >= $1".to_string()];
    let mut params: Vec<Box<dyn tokio_postgres::types::ToSql + Sync + Send>> = vec![Box::new(start)];
    let mut idx = 2u32;

    if let Some(e) = end {
        conditions.push(format!("ur.created_at < ${idx}"));
        params.push(Box::new(e));
        idx += 1;
    }
    if let Some(ref u) = q.username {
        let trimmed = u.trim();
        if !trimmed.is_empty() {
            conditions.push(format!("u.username ILIKE ${idx}"));
            params.push(Box::new(format!("%{trimmed}%")));
            idx += 1;
        }
    }
    if let Some(ref m) = q.model_id {
        conditions.push(format!("ur.model_id = ${idx}"));
        params.push(Box::new(m.clone()));
        idx += 1;
    }
    if let Some(d) = q.department_id {
        conditions.push(format!("ur.department_id = ${idx}"));
        params.push(Box::new(d));
        idx += 1;
    }
    let _ = idx;

    let where_clause = format!("WHERE {}", conditions.join(" AND "));
    let param_refs: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
        params.iter().map(|p| p.as_ref() as &(dyn tokio_postgres::types::ToSql + Sync)).collect();

    // 总数查询
    let count_sql = format!(
        "SELECT COUNT(*) FROM usage_records ur \
         JOIN users u ON u.id = ur.user_id \
         LEFT JOIN departments d ON d.id = ur.department_id {where_clause}"
    );
    let total: i64 = client.query_one(&count_sql, &param_refs)
        .await.map_err(|e| Error::Database(e.to_string()))?.get(0);

    // 明细查询
    let data_sql = format!(
        "SELECT ur.id, u.username, ur.model_id, ur.input_tokens, ur.output_tokens, \
                ur.cost_cents, ur.created_at, d.name as dept_name \
         FROM usage_records ur \
         JOIN users u ON u.id = ur.user_id \
         LEFT JOIN departments d ON d.id = ur.department_id \
         {where_clause} ORDER BY ur.created_at DESC LIMIT {page_size} OFFSET {offset}"
    );
    let rows = client.query(&data_sql, &param_refs)
        .await.map_err(|e| Error::Database(e.to_string()))?;

    let records: Vec<_> = rows.iter().map(row_to_usage_record).collect();

    Ok(Json(json!({
        "records": records,
        "total": total,
        "page": page,
        "page_size": page_size,
    })))
}

/// 将数据库行映射为 usage_record JSON
fn row_to_usage_record(r: &tokio_postgres::Row) -> serde_json::Value {
    json!({
        "id": r.get::<_, Uuid>(0).to_string(),
        "username": r.get::<_, String>(1),
        "model_id": r.get::<_, String>(2),
        "input_tokens": r.get::<_, i32>(3),
        "output_tokens": r.get::<_, i32>(4),
        "cost_cents": r.get::<_, i32>(5),
        "created_at": r.get::<_, chrono::DateTime<Utc>>(6).to_rfc3339(),
        "department_name": r.get::<_, Option<String>>(7),
    })
}

// ============================================================================
// GET /api/departments/{id}/quota-summary — 根部门配额摘要（验收标准15）
// ============================================================================

pub async fn department_quota_summary(
    State(state): State<AppState>,
    Path(dept_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let today_start = today_start_utc();
    let month_start = month_start_utc();

    // 子部门限额累加（只查直接子部门 + quota_configs 表）
    let children_sum = query_children_limit_sum(&client, dept_id).await?;

    // 该部门自定义限额和月度预算（quota_configs 表）
    let config_row = client.query_opt(
        "SELECT daily_limit_cents, monthly_budget_cents FROM quota_configs
         WHERE scope = 'department' AND scope_id = $1",
        &[&dept_id],
    ).await.map_err(|e| Error::Database(e.to_string()))?;

    let (custom_limit, monthly_budget) = match config_row {
        Some(r) => (
            r.get::<_, Option<i32>>(0).map(|v| v as i64),
            r.get::<_, Option<i32>>(1).map(|v| v as i64),
        ),
        None => (None, None),
    };
    let is_custom = custom_limit.is_some();

    // 全局当日消耗
    let today_used = query_global_daily_usage(&client, &today_start).await?;

    // 全局本月消耗
    let (month_used, _) = query_period_usage(&client, &month_start).await?;

    Ok(Json(json!({
        "children_limit_sum_cents": children_sum,
        "custom_limit_cents": custom_limit,
        "is_custom": is_custom,
        "today_total_used_cents": today_used,
        "monthly_budget_cents": monthly_budget,
        "month_total_used_cents": month_used,
    })))
}

// ============================================================================
// 内部辅助函数
// ============================================================================

fn today_start_utc() -> chrono::DateTime<Utc> {
    let now = Utc::now();
    now.date_naive().and_hms_opt(0, 0, 0)
        .expect("valid midnight time")
        .and_utc()
}

fn month_start_utc() -> chrono::DateTime<Utc> {
    let now = Utc::now();
    chrono::NaiveDate::from_ymd_opt(now.year(), now.month(), 1)
        .expect("valid first day of month")
        .and_hms_opt(0, 0, 0)
        .expect("valid midnight time")
        .and_utc()
}

/// 查用户所属部门
async fn query_user_department(
    client: &deadpool_postgres::Object,
    user_id: Uuid,
) -> Result<Option<Uuid>> {
    let row = client.query_opt(
        "SELECT department_id FROM users WHERE id = $1",
        &[&user_id],
    ).await.map_err(|e| Error::Database(e.to_string()))?;
    Ok(row.and_then(|r| r.get(0)))
}

/// 查部门级当日消耗
async fn query_dept_daily_usage(
    client: &deadpool_postgres::Object,
    dept_id: Uuid,
    today_start: &chrono::DateTime<Utc>,
) -> Result<i64> {
    let row = client.query_one(
        "SELECT COALESCE(SUM(cost_cents), 0) FROM usage_records
         WHERE department_id = $1 AND created_at >= $2",
        &[&dept_id, today_start],
    ).await.map_err(|e| Error::Database(e.to_string()))?;
    Ok(row.get(0))
}

/// 查指定时间段的全局消耗（费用 + Token 数）
async fn query_period_usage(
    client: &deadpool_postgres::Object,
    since: &chrono::DateTime<Utc>,
) -> Result<(i64, i64)> {
    let row = client.query_one(
        "SELECT COALESCE(SUM(cost_cents), 0), COALESCE(SUM(input_tokens + output_tokens), 0)
         FROM usage_records WHERE created_at >= $1",
        &[since],
    ).await.map_err(|e| Error::Database(e.to_string()))?;
    Ok((row.get(0), row.get(1)))
}

/// 查全局当日费用消耗（query_period_usage 的费用分量快捷方式）
async fn query_global_daily_usage(
    client: &deadpool_postgres::Object,
    today_start: &chrono::DateTime<Utc>,
) -> Result<i64> {
    let (cost, _) = query_period_usage(client, today_start).await?;
    Ok(cost)
}

/// 沿 parent_id 链向上找根部门（parent_id IS NULL）
/// 最多遍历 20 层防止循环引用
async fn find_root_department(
    client: &deadpool_postgres::Object,
    dept_id: Uuid,
) -> Result<Uuid> {
    let mut current = dept_id;
    for _ in 0..20 {
        let row = client.query_opt(
            "SELECT parent_id FROM departments WHERE id = $1",
            &[&current],
        ).await.map_err(|e| Error::Database(e.to_string()))?;

        match row.and_then(|r| r.get::<_, Option<Uuid>>(0)) {
            Some(parent) => current = parent,
            None => return Ok(current), // parent_id IS NULL = 根部门
        }
    }
    // 超过 20 层视为数据异常，返回当前节点作为根
    Ok(current)
}

/// 查部门限额（只查 quota_configs 表）
async fn query_department_daily_limit(
    client: &deadpool_postgres::Object,
    dept_id: Uuid,
) -> Result<Option<i64>> {
    let row = client.query_opt(
        "SELECT daily_limit_cents FROM quota_configs
         WHERE scope = 'department' AND scope_id = $1",
        &[&dept_id],
    ).await.map_err(|e| Error::Database(e.to_string()))?;

    Ok(row.and_then(|r| r.get::<_, Option<i32>>(0).map(|v| v as i64)))
}

/// 查子部门限额累加值（验收标准15）
/// 只查直接子部门的 quota_configs，不递归
async fn query_children_limit_sum(
    client: &deadpool_postgres::Object,
    parent_id: Uuid,
) -> Result<i64> {
    let row = client.query_one(
        "SELECT COALESCE(SUM(qc.daily_limit_cents), 0)
         FROM departments d
         JOIN quota_configs qc ON qc.scope = 'department' AND qc.scope_id = d.id
         WHERE d.parent_id = $1",
        &[&parent_id],
    ).await.map_err(|e| Error::Database(e.to_string()))?;
    Ok(row.get(0))
}

/// 根据模型单价计算费用（分），未定价模型按 0 计费
async fn calculate_cost(
    client: &deadpool_postgres::Object,
    model_id: &str,
    input_tokens: i32,
    output_tokens: i32,
) -> Result<(Option<Uuid>, i32)> {
    let row = client.query_opt(
        "SELECT id, input_price_per_1k_cents, output_price_per_1k_cents
         FROM model_configs WHERE model_id = $1",
        &[&model_id],
    ).await.map_err(|e| Error::Database(e.to_string()))?;

    let (config_id, input_price, output_price) = match row {
        Some(r) => (
            Some(r.get::<_, Uuid>(0)),
            r.get::<_, Option<i32>>(1).unwrap_or(0),
            r.get::<_, Option<i32>>(2).unwrap_or(0),
        ),
        None => (None, 0, 0),
    };

    let cost = (input_tokens as i64 * input_price as i64 / 1000
              + output_tokens as i64 * output_price as i64 / 1000) as i32;

    Ok((config_id, cost))
}

// ============================================================================
// 费用预警检查（需求 4#7：部门费用达到限额 80% 时触发告警）
// ============================================================================

const QUOTA_WARNING_THRESHOLD: f64 = 0.8;

/// 检查部门当日费用是否达到限额的 80%，达到则触发告警。
///
/// 设计决策：
/// - 独立函数，不污染 report_usage 的主流程
/// - 查询失败时静默返回（预警是增强功能，不应阻塞计费）
/// - 使用已有的 query_dept_daily_usage 和 query_department_daily_limit 复用查询逻辑
async fn check_quota_warning(
    pool: &deadpool_postgres::Pool,
    dept_id: Uuid,
) -> std::result::Result<(), String> {
    let client = pool.get().await.map_err(|e| e.to_string())?;
    let today = today_start_utc();

    let limit = match query_department_daily_limit(&client, dept_id).await {
        Ok(Some(l)) if l > 0 => l,
        _ => return Ok(()), // 无限额或查询失败，跳过
    };

    let usage = query_dept_daily_usage(&client, dept_id, &today).await
        .map_err(|e| e.to_string())?;

    let ratio = usage as f64 / limit as f64;
    if ratio < QUOTA_WARNING_THRESHOLD {
        return Ok(());
    }

    let dept_name = client
        .query_opt("SELECT name FROM departments WHERE id = $1", &[&dept_id])
        .await
        .map_err(|e| e.to_string())?
        .map(|r| r.get::<_, String>(0))
        .unwrap_or_else(|| "未知部门".into());

    let pct = (ratio * 100.0) as i32;
    let trigger = AlertTrigger {
        event_type: "quota_exceeded".into(),
        severity: if ratio >= 1.0 { "critical".into() } else { "high".into() },
        detail: format!(
            "{} 当日费用消耗已达限额 {}%（{} / {} 分）",
            dept_name, pct, usage, limit
        ),
        event_data: Some(json!({
            "department_id": dept_id,
            "department_name": dept_name,
            "usage_cents": usage,
            "limit_cents": limit,
            "ratio_pct": pct,
        })),
    };

    alerts::trigger_alert(pool, &trigger).await?;
    Ok(())
}
