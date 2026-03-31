//! 部门管理扩展 Handler
//!
//! 新增端点（基础 CRUD 仍在 routes.rs，TODO: 后续迁移）：
//! - GET  /api/departments/{id}              — 部门详情
//! - GET  /api/departments/{id}/members      — 部门成员列表
//! - GET  /api/departments/{id}/model-whitelist  — 获取模型白名单
//! - PUT  /api/departments/{id}/model-whitelist  — 更新模型白名单

use crate::error::{Error, Result};
use crate::models::{DepartmentMembersQuery, UpdateModelWhitelistRequest};
use crate::routes::write_audit_log;
use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde_json::json;
use uuid::Uuid;

// ============================================================================
// GET /api/departments/{id} — 部门详情
// ============================================================================

pub async fn get_department_detail(
    State(state): State<AppState>,
    Path(dept_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let dept = query_department_by_id(&client, dept_id).await?;
    let whitelist_count = query_whitelist_count(&client, dept_id).await?;

    let mut result = dept;
    result["model_whitelist_count"] = json!(whitelist_count);

    Ok(Json(result))
}

// ============================================================================
// GET /api/departments/{id}/members — 部门成员列表
// ============================================================================

pub async fn get_department_members(
    State(state): State<AppState>,
    Path(dept_id): Path<Uuid>,
    Query(params): Query<DepartmentMembersQuery>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    // 验证部门存在
    verify_department_exists(&client, dept_id).await?;

    let (sql, query_params) = build_members_query(dept_id, &params);
    let param_refs: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
        query_params.iter().map(|p| p.as_ref() as &(dyn tokio_postgres::types::ToSql + Sync)).collect();

    let rows = client.query(&sql, &param_refs).await
        .map_err(|e| Error::Database(e.to_string()))?;

    let members: Vec<_> = rows.iter().map(|r| {
        json!({
            "id": r.get::<_, Uuid>(0),
            "username": r.get::<_, String>(1),
            "email": r.get::<_, Option<String>>(2),
            "created_at": r.get::<_, chrono::DateTime<chrono::Utc>>(3),
        })
    }).collect();

    Ok(Json(json!({ "members": members, "total": members.len() })))
}

// ============================================================================
// GET /api/departments/{id}/model-whitelist — 获取模型白名单
// ============================================================================

pub async fn get_model_whitelist(
    State(state): State<AppState>,
    Path(dept_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    verify_department_exists(&client, dept_id).await?;

    let rows = client.query(
        "SELECT mc.id, mc.model_id, mc.display_name, mc.provider, mc.enabled
         FROM department_model_whitelist dmw
         JOIN model_configs mc ON mc.id = dmw.model_config_id
         WHERE dmw.department_id = $1
         ORDER BY mc.sort_order, mc.display_name",
        &[&dept_id],
    ).await.map_err(|e| Error::Database(e.to_string()))?;

    let models: Vec<_> = rows.iter().map(|r| {
        json!({
            "id": r.get::<_, Uuid>(0),
            "model_id": r.get::<_, String>(1),
            "display_name": r.get::<_, String>(2),
            "provider": r.get::<_, String>(3),
            "enabled": r.get::<_, bool>(4),
        })
    }).collect();

    Ok(Json(json!({ "models": models })))
}

// ============================================================================
// PUT /api/departments/{id}/model-whitelist — 更新模型白名单（全量替换）
// ============================================================================

pub async fn update_model_whitelist(
    State(state): State<AppState>,
    Path(dept_id): Path<Uuid>,
    Json(payload): Json<UpdateModelWhitelistRequest>,
) -> Result<Json<serde_json::Value>> {
    let mut client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let dept_name = verify_department_exists_returning_name(&client, dept_id).await?;

    // 事务：先删后插，保证原子性
    let tx = client.transaction().await
        .map_err(|e| Error::Database(e.to_string()))?;

    tx.execute(
        "DELETE FROM department_model_whitelist WHERE department_id = $1",
        &[&dept_id],
    ).await.map_err(|e| Error::Database(e.to_string()))?;

    for model_id in &payload.model_config_ids {
        tx.execute(
            "INSERT INTO department_model_whitelist (department_id, model_config_id)
             VALUES ($1, $2) ON CONFLICT DO NOTHING",
            &[&dept_id, model_id],
        ).await.map_err(|e| Error::Database(e.to_string()))?;
    }

    tx.commit().await.map_err(|e| Error::Database(e.to_string()))?;

    // 审计日志
    write_audit_log(
        &client,
        Uuid::nil(),
        "update_department_model_whitelist",
        &format!(
            "更新部门 {} 的模型白名单，共 {} 个模型",
            dept_name,
            payload.model_config_ids.len()
        ),
    ).await;

    Ok(Json(json!({
        "message": "模型白名单已更新",
        "count": payload.model_config_ids.len(),
    })))
}

// ============================================================================
// 内部辅助函数
// ============================================================================

/// 查询单个部门的完整信息（含成员数和父部门名称）
async fn query_department_by_id(
    client: &deadpool_postgres::Object,
    dept_id: Uuid,
) -> Result<serde_json::Value> {
    let row = client.query_opt(
        "SELECT d.id, d.name, d.description, d.parent_id, d.token_quota_enabled,
                d.token_quota_per_day, d.created_at, d.updated_at,
                COUNT(u.id) as member_count,
                p.name as parent_name
         FROM departments d
         LEFT JOIN users u ON u.department_id = d.id
         LEFT JOIN departments p ON p.id = d.parent_id
         WHERE d.id = $1
         GROUP BY d.id, d.name, d.description, d.parent_id, d.token_quota_enabled,
                  d.token_quota_per_day, d.created_at, d.updated_at, p.name",
        &[&dept_id],
    ).await.map_err(|e| Error::Database(e.to_string()))?
     .ok_or(Error::DepartmentNotFound)?;

    let quota_enabled: bool = row.get(4);
    Ok(json!({
        "id": row.get::<_, Uuid>(0),
        "name": row.get::<_, String>(1),
        "description": row.get::<_, Option<String>>(2),
        "parent_id": row.get::<_, Option<Uuid>>(3),
        "token_quota_enabled": quota_enabled,
        "token_quota_per_day": if quota_enabled { row.get::<_, Option<i32>>(5) } else { None },
        "created_at": row.get::<_, chrono::DateTime<chrono::Utc>>(6),
        "updated_at": row.get::<_, chrono::DateTime<chrono::Utc>>(7),
        "member_count": row.get::<_, i64>(8),
        "parent_name": row.get::<_, Option<String>>(9),
    }))
}

/// 查询部门模型白名单数量
async fn query_whitelist_count(
    client: &deadpool_postgres::Object,
    dept_id: Uuid,
) -> Result<i64> {
    let row = client.query_one(
        "SELECT COUNT(*) FROM department_model_whitelist WHERE department_id = $1",
        &[&dept_id],
    ).await.map_err(|e| Error::Database(e.to_string()))?;
    Ok(row.get(0))
}

/// 验证部门存在，不存在则返回 DepartmentNotFound
async fn verify_department_exists(
    client: &deadpool_postgres::Object,
    dept_id: Uuid,
) -> Result<()> {
    client.query_opt(
        "SELECT 1 FROM departments WHERE id = $1",
        &[&dept_id],
    ).await.map_err(|e| Error::Database(e.to_string()))?
     .ok_or(Error::DepartmentNotFound)?;
    Ok(())
}

/// 验证部门存在并返回名称
async fn verify_department_exists_returning_name(
    client: &deadpool_postgres::Object,
    dept_id: Uuid,
) -> Result<String> {
    let row = client.query_opt(
        "SELECT name FROM departments WHERE id = $1",
        &[&dept_id],
    ).await.map_err(|e| Error::Database(e.to_string()))?
     .ok_or(Error::DepartmentNotFound)?;
    Ok(row.get(0))
}

/// 构建成员查询 SQL（支持搜索）
fn build_members_query(
    dept_id: Uuid,
    params: &DepartmentMembersQuery,
) -> (String, Vec<Box<dyn tokio_postgres::types::ToSql + Sync + Send>>) {
    let mut sql = String::from(
        "SELECT u.id, u.username, u.email, u.created_at
         FROM users u WHERE u.department_id = $1"
    );
    let mut query_params: Vec<Box<dyn tokio_postgres::types::ToSql + Sync + Send>> =
        vec![Box::new(dept_id)];

    if let Some(ref search) = params.search {
        let trimmed = search.trim();
        if !trimmed.is_empty() {
            sql.push_str(" AND (u.username ILIKE $2 OR u.email ILIKE $2)");
            query_params.push(Box::new(format!("%{}%", trimmed)));
        }
    }

    sql.push_str(" ORDER BY u.username ASC");
    (sql, query_params)
}
