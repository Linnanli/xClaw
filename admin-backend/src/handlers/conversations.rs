//! 对话审计 Handler
//!
//! 端点：
//! - GET  /api/conversations          — 对话列表（摘要，不含消息内容）
//! - GET  /api/conversations/{id}     — 对话详情（含完整消息流）
//! - GET  /api/conversations/stats    — 对话统计

use crate::error::{Error, Result};
use crate::models::ConversationQuery;
use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde_json::json;
use uuid::Uuid;

/// GET /api/conversations — 对话列表（仅摘要）
pub async fn get_conversations(
    State(state): State<AppState>,
    Query(params): Query<ConversationQuery>,
) -> Result<Json<serde_json::Value>> {
    let client = state
        .db_pool
        .get()
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let page = params.page.unwrap_or(1).max(1);
    let page_size = params.page_size.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1) * page_size;

    let (where_clause, query_params) = build_conversations_filter(&params);
    let param_refs: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = query_params
        .iter()
        .map(|p| &**p as &(dyn tokio_postgres::types::ToSql + Sync))
        .collect();

    let count_sql = format!(
        "SELECT COUNT(*) FROM conversations c JOIN users u ON u.id = c.user_id {}",
        where_clause
    );
    let total: i64 = client
        .query_one(&count_sql, &param_refs)
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .get(0);

    let data_sql = format!(
        "SELECT c.id, u.username, c.topic, c.message_count, c.total_tokens,
                c.model_id, c.dlp_flagged, c.created_at
         FROM conversations c
         JOIN users u ON u.id = c.user_id
         {} ORDER BY c.created_at DESC LIMIT {} OFFSET {}",
        where_clause, page_size, offset
    );
    let rows = client
        .query(&data_sql, &param_refs)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let conversations: Vec<_> = rows.iter().map(row_to_conversation_summary).collect();

    Ok(Json(json!({
        "data": conversations,
        "total": total,
        "page": page,
        "page_size": page_size,
    })))
}

/// GET /api/conversations/{id} — 对话详情（含完整消息流）
pub async fn get_conversation_detail(
    State(state): State<AppState>,
    Path(conv_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    let client = state
        .db_pool
        .get()
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let conv = client
        .query_opt(
            "SELECT c.id, u.username, c.topic, c.message_count, c.total_tokens,
                    c.model_id, c.dlp_flagged, c.dlp_details, c.created_at
             FROM conversations c
             JOIN users u ON u.id = c.user_id
             WHERE c.id = $1",
            &[&conv_id],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .ok_or_else(|| Error::NotFound("对话记录不存在".into()))?;

    let messages = client
        .query(
            "SELECT id, role, content, attachments, model_id, input_tokens, output_tokens, created_at
             FROM conversation_messages
             WHERE conversation_id = $1
             ORDER BY created_at ASC",
            &[&conv_id],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let msgs: Vec<_> = messages
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<_, Uuid>(0),
                "role": r.get::<_, String>(1),
                "content": r.get::<_, String>(2),
                "attachments": r.get::<_, Option<serde_json::Value>>(3).unwrap_or_else(|| json!([])),
                "model_id": r.get::<_, Option<String>>(4),
                "input_tokens": r.get::<_, i32>(5),
                "output_tokens": r.get::<_, i32>(6),
                "created_at": r.get::<_, chrono::DateTime<chrono::Utc>>(7),
            })
        })
        .collect();

    Ok(Json(json!({
        "id": conv.get::<_, Uuid>(0),
        "username": conv.get::<_, String>(1),
        "topic": conv.get::<_, String>(2),
        "message_count": conv.get::<_, i32>(3),
        "total_tokens": conv.get::<_, i32>(4),
        "model_id": conv.get::<_, Option<String>>(5),
        "dlp_flagged": conv.get::<_, bool>(6),
        "dlp_details": conv.get::<_, Option<String>>(7),
        "created_at": conv.get::<_, chrono::DateTime<chrono::Utc>>(8),
        "messages": msgs,
    })))
}

/// GET /api/conversations/stats — 对话统计
pub async fn get_conversation_stats(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>> {
    let client = state
        .db_pool
        .get()
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let row = client
        .query_one(
            "SELECT
                COUNT(*) FILTER (WHERE created_at >= CURRENT_DATE) as today_count,
                COALESCE(SUM(total_tokens) FILTER (WHERE created_at >= CURRENT_DATE), 0) as today_tokens,
                COUNT(*) FILTER (WHERE dlp_flagged = true AND created_at >= CURRENT_DATE) as today_dlp,
                COUNT(DISTINCT user_id) FILTER (WHERE created_at >= CURRENT_DATE) as today_users
             FROM conversations",
            &[],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    Ok(Json(json!({
        "today_count": row.get::<_, i64>(0),
        "today_tokens": row.get::<_, i64>(1),
        "today_dlp_flagged": row.get::<_, i64>(2),
        "today_active_users": row.get::<_, i64>(3),
    })))
}

// ============================================================================
// client-reports 中 conversation 类型的处理函数
// ============================================================================

use crate::models::ConversationReportPayload;

/// 处理 report_type="conversation" 的上报数据。
///
/// 由 post_client_reports 调用，幂等：client_conversation_id 重复时跳过。
/// 写入成功后，返回 assistant 消息的 Token 汇总，供调用方触发费用上报。
pub async fn ingest_conversation(
    client: &deadpool_postgres::Object,
    payload: &ConversationReportPayload,
) -> std::result::Result<Option<TokenSummary>, String> {
    // 幂等检查
    let existing = client
        .query_opt(
            "SELECT id FROM conversations WHERE client_conversation_id = $1",
            &[&payload.client_conversation_id],
        )
        .await
        .map_err(|e| e.to_string())?;

    if existing.is_some() {
        return Ok(None); // 已存在，跳过
    }

    let total_tokens: i32 = payload
        .messages
        .iter()
        .map(|m| m.input_tokens + m.output_tokens)
        .sum();

    let conv_id = Uuid::new_v4();
    let topic = payload.topic.as_deref().unwrap_or("");
    let now = chrono::Utc::now();

    client
        .execute(
            "INSERT INTO conversations (id, client_conversation_id, user_id, topic, model_id,
                                        message_count, total_tokens, dlp_flagged, dlp_details, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
            &[
                &conv_id,
                &payload.client_conversation_id,
                &payload.user_id,
                &topic,
                &payload.model_id,
                &(payload.messages.len() as i32),
                &total_tokens,
                &payload.dlp_flagged.unwrap_or(false),
                &payload.dlp_details,
                &now,
                &now,
            ],
        )
        .await
        .map_err(|e| e.to_string())?;

    for msg in &payload.messages {
        let attachments = serde_json::to_value(&msg.attachments).map_err(|e| e.to_string())?;
        client
            .execute(
                "INSERT INTO conversation_messages (conversation_id, role, content, attachments, model_id, input_tokens, output_tokens)
                 VALUES ($1, $2, $3, $4, $5, $6, $7)",
                &[
                    &conv_id,
                    &msg.role,
                    &msg.content,
                    &attachments,
                    &msg.model_id,
                    &msg.input_tokens,
                    &msg.output_tokens,
                ],
            )
            .await
            .map_err(|e| e.to_string())?;
    }

    // 汇总 assistant 消息的 Token，供调用方触发费用上报
    let summary = aggregate_assistant_tokens(payload);
    Ok(Some(summary))
}

/// assistant 消息的 Token 汇总（按模型分组）。
#[derive(Debug)]
pub struct TokenSummary {
    /// (model_id, input_tokens, output_tokens)
    pub by_model: Vec<(String, i32, i32)>,
}

/// 将 assistant 消息的 Token 按模型 ID 汇总。
fn aggregate_assistant_tokens(payload: &ConversationReportPayload) -> TokenSummary {
    use std::collections::HashMap;

    let mut map: HashMap<String, (i32, i32)> = HashMap::new();
    let fallback_model = payload.model_id.clone().unwrap_or_default();

    for msg in &payload.messages {
        if msg.role != "assistant" {
            continue;
        }
        let model = msg
            .model_id
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(&fallback_model)
            .to_string();
        let entry = map.entry(model).or_default();
        entry.0 += msg.input_tokens;
        entry.1 += msg.output_tokens;
    }

    TokenSummary {
        by_model: map.into_iter().map(|(m, (i, o))| (m, i, o)).collect(),
    }
}

// ============================================================================
// 内部辅助函数
// ============================================================================

fn row_to_conversation_summary(row: &tokio_postgres::Row) -> serde_json::Value {
    json!({
        "id": row.get::<_, Uuid>(0),
        "username": row.get::<_, String>(1),
        "topic": row.get::<_, String>(2),
        "message_count": row.get::<_, i32>(3),
        "total_tokens": row.get::<_, i32>(4),
        "model_id": row.get::<_, Option<String>>(5),
        "dlp_flagged": row.get::<_, bool>(6),
        "created_at": row.get::<_, chrono::DateTime<chrono::Utc>>(7),
    })
}

fn build_conversations_filter(
    params: &ConversationQuery,
) -> (
    String,
    Vec<Box<dyn tokio_postgres::types::ToSql + Sync + Send>>,
) {
    let mut conditions: Vec<String> = Vec::new();
    let mut query_params: Vec<Box<dyn tokio_postgres::types::ToSql + Sync + Send>> = Vec::new();
    let mut idx = 1;

    if let Some(ref username) = params.username {
        let trimmed = username.trim();
        if !trimmed.is_empty() {
            conditions.push(format!("u.username ILIKE ${}", idx));
            query_params.push(Box::new(format!("%{}%", trimmed)));
            idx += 1;
        }
    }

    if let Some(dlp) = params.dlp_flagged {
        conditions.push(format!("c.dlp_flagged = ${}", idx));
        query_params.push(Box::new(dlp));
        idx += 1;
    }

    if let Some(ref search) = params.search {
        let trimmed = search.trim();
        if !trimmed.is_empty() {
            // 同一参数用于两个 ILIKE 条件，需要 push 两次
            conditions.push(format!(
                "(c.topic ILIKE ${} OR u.username ILIKE ${})",
                idx,
                idx + 1
            ));
            let pattern = format!("%{}%", trimmed);
            query_params.push(Box::new(pattern.clone()));
            query_params.push(Box::new(pattern));
            idx += 2;
        }
    }
    let _ = idx; // 最后一次赋值后不再使用，显式忽略

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    (where_clause, query_params)
}
