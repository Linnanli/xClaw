//! 操作审批流 Handler
//!
//! 端点：
//! - POST /api/approvals              — 创建审批工单
//! - GET  /api/approvals              — 审批工单列表
//! - GET  /api/approvals/stats        — 审批统计
//! - PUT  /api/approvals/{id}/review  — 审批（批准/拒绝）
//! - GET  /api/approvals/{id}/check   — 检查审批状态（客户端用）

use crate::error::{Error, Result};
use crate::models::{ApprovalQuery, CreateApprovalRequest, ReviewApprovalRequest};
use crate::routes::write_audit_log;
use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde_json::json;
use uuid::Uuid;

/// POST /api/approvals — 创建审批工单
pub async fn create_approval(
    State(state): State<AppState>,
    Json(payload): Json<CreateApprovalRequest>,
) -> Result<Json<serde_json::Value>> {
    let client = state
        .db_pool
        .get()
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let row = client
        .query_one(
            "INSERT INTO approval_tickets (applicant_id, operation_rule_id, operation_type, operation_name, reason)
             VALUES ($1, $2, $3, $4, $5)
             RETURNING id, created_at, expires_at",
            &[
                &payload.applicant_id,
                &payload.operation_rule_id,
                &payload.operation_type,
                &payload.operation_name,
                &payload.reason,
            ],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let id: Uuid = row.get(0);

    write_audit_log(
        &client,
        payload.applicant_id,
        "create_approval",
        &format!(
            "创建审批工单: {} ({})",
            payload.operation_name, payload.operation_type
        ),
    )
    .await;

    Ok(Json(json!({
        "id": id,
        "created_at": row.get::<_, chrono::DateTime<chrono::Utc>>(1),
        "expires_at": row.get::<_, chrono::DateTime<chrono::Utc>>(2),
        "message": "审批工单已创建",
    })))
}

/// GET /api/approvals — 审批工单列表
pub async fn get_approvals(
    State(state): State<AppState>,
    Query(params): Query<ApprovalQuery>,
) -> Result<Json<serde_json::Value>> {
    let client = state
        .db_pool
        .get()
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    // 先将过期的 pending 工单标记为 expired
    mark_expired_tickets(&client).await;

    let page = params.page.unwrap_or(1).max(1);
    let page_size = params.page_size.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1) * page_size;

    let (where_clause, query_params) = build_approval_filter(&params);
    let param_refs: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = query_params
        .iter()
        .map(|p| &**p as &(dyn tokio_postgres::types::ToSql + Sync))
        .collect();

    let count_sql = format!(
        "SELECT COUNT(*) FROM approval_tickets t JOIN users u ON u.id = t.applicant_id {}",
        where_clause
    );
    let total: i64 = client
        .query_one(&count_sql, &param_refs)
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .get(0);

    let data_sql = format!(
        "SELECT t.id, u.username, t.operation_type, t.operation_name, t.reason,
            t.status, t.review_comment, t.reviewed_at, t.expires_at, t.created_at
         FROM approval_tickets t
         JOIN users u ON u.id = t.applicant_id
         {} ORDER BY t.created_at DESC LIMIT {} OFFSET {}",
        where_clause, page_size, offset
    );
    let rows = client
        .query(&data_sql, &param_refs)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let tickets: Vec<_> = rows.iter().map(row_to_approval_ticket).collect();

    Ok(Json(json!({
        "data": tickets,
        "total": total,
        "page": page,
        "page_size": page_size,
    })))
}

/// GET /api/approvals/stats — 审批统计
pub async fn get_approval_stats(State(state): State<AppState>) -> Result<Json<serde_json::Value>> {
    let client = state
        .db_pool
        .get()
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    mark_expired_tickets(&client).await;

    let row = client
        .query_one(
            "SELECT
            COUNT(*) FILTER (WHERE status = 'pending') as pending,
            COUNT(*) FILTER (WHERE status = 'approved') as approved,
            COUNT(*) FILTER (WHERE status = 'rejected') as rejected,
            COUNT(*) FILTER (WHERE status = 'expired') as expired
         FROM approval_tickets",
            &[],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    Ok(Json(json!({
        "pending": row.get::<_, i64>(0),
        "approved": row.get::<_, i64>(1),
        "rejected": row.get::<_, i64>(2),
        "expired": row.get::<_, i64>(3),
    })))
}

/// PUT /api/approvals/{id}/review — 审批（批准/拒绝）
pub async fn review_approval(
    State(state): State<AppState>,
    Path(ticket_id): Path<Uuid>,
    Json(payload): Json<ReviewApprovalRequest>,
) -> Result<Json<serde_json::Value>> {
    let new_status = match payload.action.as_str() {
        "approve" => "approved",
        "reject" => "rejected",
        _ => return Err(Error::Validation("action 必须为 approve 或 reject".into())),
    };

    let client = state
        .db_pool
        .get()
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let existing = client
        .query_opt(
            "SELECT status, operation_name FROM approval_tickets WHERE id = $1",
            &[&ticket_id],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .ok_or_else(|| Error::NotFound("审批工单不存在".into()))?;

    let current_status: String = existing.get(0);
    let op_name: String = existing.get(1);

    if current_status != "pending" {
        return Err(Error::Validation(format!(
            "工单状态为 {}，无法审批",
            current_status
        )));
    }

    let now = chrono::Utc::now();
    client.execute(
        "UPDATE approval_tickets SET status = $1, review_comment = $2, reviewed_at = $3, reviewer_id = $4 WHERE id = $5",
        &[&new_status, &payload.comment, &now, &Uuid::nil(), &ticket_id],
    ).await.map_err(|e| Error::Database(e.to_string()))?;

    let action_label = if new_status == "approved" {
        "批准"
    } else {
        "拒绝"
    };
    write_audit_log(
        &client,
        Uuid::nil(),
        "review_approval",
        &format!("{}审批工单: {}", action_label, op_name),
    )
    .await;

    Ok(Json(
        json!({ "message": format!("工单已{}", action_label) }),
    ))
}

/// GET /api/approvals/{id}/check — 检查审批状态（客户端用）
pub async fn check_approval(
    State(state): State<AppState>,
    Path(ticket_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    let client = state
        .db_pool
        .get()
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let row = client
        .query_opt(
            "SELECT status, expires_at, reviewed_at FROM approval_tickets WHERE id = $1",
            &[&ticket_id],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .ok_or_else(|| Error::NotFound("审批工单不存在".into()))?;

    let status: String = row.get(0);
    let expires_at: chrono::DateTime<chrono::Utc> = row.get(1);
    let is_valid = status == "approved" && chrono::Utc::now() < expires_at;

    Ok(Json(json!({
        "status": status,
        "expires_at": expires_at,
        "is_valid": is_valid,
    })))
}

// ============================================================================
// 内部辅助函数
// ============================================================================

/// 将过期的 pending 工单标记为 expired（lazy evaluation，无需定时任务）
async fn mark_expired_tickets(client: &deadpool_postgres::Object) {
    let _ = client.execute(
        "UPDATE approval_tickets SET status = 'expired' WHERE status = 'pending' AND expires_at < NOW()",
        &[],
    ).await;
}

fn row_to_approval_ticket(row: &tokio_postgres::Row) -> serde_json::Value {
    json!({
        "id": row.get::<_, Uuid>(0),
        "applicant": row.get::<_, String>(1),
        "operation_type": row.get::<_, String>(2),
        "operation_name": row.get::<_, String>(3),
        "reason": row.get::<_, Option<String>>(4),
        "status": row.get::<_, String>(5),
        "review_comment": row.get::<_, Option<String>>(6),
        "reviewed_at": row.get::<_, Option<chrono::DateTime<chrono::Utc>>>(7),
        "expires_at": row.get::<_, chrono::DateTime<chrono::Utc>>(8),
        "created_at": row.get::<_, chrono::DateTime<chrono::Utc>>(9),
    })
}

fn build_approval_filter(
    params: &ApprovalQuery,
) -> (
    String,
    Vec<Box<dyn tokio_postgres::types::ToSql + Sync + Send>>,
) {
    let mut conditions: Vec<String> = Vec::new();
    let mut query_params: Vec<Box<dyn tokio_postgres::types::ToSql + Sync + Send>> = Vec::new();

    if let Some(ref status) = params.status {
        let trimmed = status.trim();
        if !trimmed.is_empty() {
            conditions.push("t.status = $1".to_string());
            query_params.push(Box::new(trimmed.to_string()));
        }
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    (where_clause, query_params)
}

/// 检查审批超时催办：24 小时未处理的 pending 工单触发 approval_timeout 告警（需求 21.6）。
///
/// 由 main.rs 中的后台定时任务每小时调用一次。
pub async fn check_approval_timeouts(pool: &deadpool_postgres::Pool) {
    let client = match pool.get().await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "approval timeout check: failed to get db connection");
            return;
        }
    };

    let rows = match client
        .query(
            "SELECT t.id, u.username, t.operation_name
             FROM approval_tickets t
             JOIN users u ON u.id = t.applicant_id
             WHERE t.status = 'pending'
               AND t.created_at < NOW() - INTERVAL '24 hours'
               AND t.expires_at > NOW()",
            &[],
        )
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, "approval timeout check: query failed");
            return;
        }
    };

    for row in &rows {
        let ticket_id: uuid::Uuid = row.get(0);
        let applicant: String = row.get(1);
        let op_name: String = row.get(2);

        let trigger = crate::models::AlertTrigger {
            event_type: "approval_timeout".into(),
            severity: "medium".into(),
            detail: format!(
                "审批工单超过 24 小时未处理：{} 申请的「{}」（工单 ID: {}）",
                applicant, op_name, ticket_id
            ),
            event_data: Some(serde_json::json!({
                "ticket_id": ticket_id,
                "applicant": applicant,
                "operation_name": op_name,
            })),
        };

        if let Err(e) = crate::handlers::alerts::trigger_alert(pool, &trigger).await {
            tracing::warn!(error = %e, "Failed to trigger approval timeout alert");
        }
    }

    if !rows.is_empty() {
        tracing::info!(
            count = rows.len(),
            "approval timeout check: triggered alerts"
        );
    }
}
