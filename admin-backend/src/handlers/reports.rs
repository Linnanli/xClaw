//! 统计报表 Handler
//!
//! 端点：
//! - GET /api/reports/ai-usage      — AI 使用量趋势（日/周/月维度）
//! - GET /api/reports/model-cost    — 各模型费用与调用量统计
//! - GET /api/reports/dept-ranking  — 部门 AI 使用量排行

use crate::error::{Error, Result};
use crate::AppState;
use axum::{
    extract::{Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

// ── 时间维度 ──────────────────────────────────────────────────────────────────

/// 时间维度枚举 — 作为 SQL 片段的类型安全白名单。
///
/// DATE_TRUNC 关键字和 INTERVAL 字面量无法通过 `$1` 参数化（pg 协议限制），
/// 用枚举确保拼入 SQL 的值只能来自编译期常量，不受用户输入影响。
#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Period {
    Day,
    Week,
    Month,
    #[default]
    #[serde(other)]
    Unknown,
}

impl Period {
    fn trunc(&self) -> &'static str {
        match self {
            Period::Week => "week",
            Period::Month => "month",
            _ => "day",
        }
    }

    fn interval(&self) -> &'static str {
        match self {
            Period::Week => "7 days",
            Period::Month => "30 days",
            _ => "1 day",
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ReportQuery {
    #[serde(default)]
    pub period: Period,
    /// 返回的数据点数量，默认 7
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_limit() -> i64 {
    7
}

// ── 查询结果结构体 ─────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, sqlx::FromRow)]
struct AiUsageRow {
    period: chrono::DateTime<chrono::Utc>,
    conversation_count: i64,
    total_tokens: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct ModelCostRow {
    model_id: String,
    display_name: String,
    call_count: i64,
    input_tokens: i64,
    output_tokens: i64,
    cost_cents: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct DeptRankingRow {
    department_id: uuid::Uuid,
    department_name: String,
    call_count: i64,
    total_tokens: i64,
    cost_cents: i64,
}

// ── Handlers ──────────────────────────────────────────────────────────────────

/// GET /api/reports/ai-usage — AI 使用量趋势
pub async fn get_ai_usage(
    State(state): State<AppState>,
    Query(params): Query<ReportQuery>,
) -> Result<Json<serde_json::Value>> {
    let trunc = params.period.trunc();

    // DATE_TRUNC 关键字无法参数化（pg 协议限制），trunc 来自 Period 枚举
    let sql = format!(
        "SELECT
            DATE_TRUNC('{trunc}', created_at) AS period,
            COUNT(*)                          AS conversation_count,
            COALESCE(SUM(total_tokens), 0)    AS total_tokens
         FROM conversations
         GROUP BY period
         ORDER BY period DESC
         LIMIT $1"
    );

    let data = sqlx::query_as::<_, AiUsageRow>(&sql)
        .bind(params.limit)
        .fetch_all(&state.sqlx_pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    Ok(Json(json!({ "period": trunc, "data": data })))
}

/// GET /api/reports/model-cost — 各模型费用与调用量统计
pub async fn get_model_cost(
    State(state): State<AppState>,
    Query(params): Query<ReportQuery>,
) -> Result<Json<serde_json::Value>> {
    let interval = params.period.interval();

    // INTERVAL 字面量无法参数化（pg 协议限制），interval 来自 Period 枚举
    let sql = format!(
        "SELECT
            ur.model_id,
            COALESCE(mc.display_name, ur.model_id)  AS display_name,
            COUNT(*)                                 AS call_count,
            COALESCE(SUM(ur.input_tokens), 0)        AS input_tokens,
            COALESCE(SUM(ur.output_tokens), 0)       AS output_tokens,
            COALESCE(SUM(ur.cost_cents), 0)          AS cost_cents
         FROM usage_records ur
         LEFT JOIN model_configs mc ON mc.id = ur.model_config_id
         WHERE ur.created_at >= NOW() - INTERVAL '{interval}'
         GROUP BY ur.model_id, mc.display_name
         ORDER BY cost_cents DESC
         LIMIT $1"
    );

    let models = sqlx::query_as::<_, ModelCostRow>(&sql)
        .bind(params.limit)
        .fetch_all(&state.sqlx_pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    Ok(Json(json!({ "period": params.period, "models": models })))
}

/// GET /api/reports/dept-ranking — 部门 AI 使用量排行
pub async fn get_dept_ranking(
    State(state): State<AppState>,
    Query(params): Query<ReportQuery>,
) -> Result<Json<serde_json::Value>> {
    let interval = params.period.interval();

    // INTERVAL 字面量无法参数化（pg 协议限制），interval 来自 Period 枚举
    let sql = format!(
        "SELECT
            d.id                                                    AS department_id,
            d.name                                                  AS department_name,
            COUNT(ur.id)                                            AS call_count,
            COALESCE(SUM(ur.input_tokens + ur.output_tokens), 0)    AS total_tokens,
            COALESCE(SUM(ur.cost_cents), 0)                         AS cost_cents
         FROM departments d
         LEFT JOIN usage_records ur
            ON ur.department_id = d.id
            AND ur.created_at >= NOW() - INTERVAL '{interval}'
         GROUP BY d.id, d.name
         ORDER BY cost_cents DESC
         LIMIT $1"
    );

    let departments = sqlx::query_as::<_, DeptRankingRow>(&sql)
        .bind(params.limit)
        .fetch_all(&state.sqlx_pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    Ok(Json(
        json!({ "period": params.period, "departments": departments }),
    ))
}
