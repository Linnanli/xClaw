//! 代码工具设置 Handler（P0 Claude Code Parity）
//!
//! 提供三组 settings 端点，使用 SQLx：
//! - `/api/settings/code-tools`    — 工具启用/禁用策略（按部门）
//! - `/api/settings/workspace-paths` — 工作区路径白名单
//! - `/api/settings/bash-rules`    — Bash 命令自定义规则

use axum::{
    extract::{Extension, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{info, instrument};
use uuid::Uuid;

use crate::{
    error::{Error, Result},
    models::TokenClaims,
    AppState,
};

// ── 请求/响应类型 ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct DepartmentQuery {
    /// 部门 ID。为空则查全局默认配置。
    pub department_id: Option<Uuid>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct CodeToolSettingRow {
    id: Uuid,
    department_id: Option<Uuid>,
    setting_type: String,
    config: serde_json::Value,
    updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSettingPayload {
    /// 部门 ID。为空则修改全局默认。
    pub department_id: Option<Uuid>,
    /// 配置 JSON（结构取决于 setting_type）
    pub config: serde_json::Value,
}

// ── 内部辅助 ────────────────────────────────────────────────────────────

async fn get_setting(
    pool: &sqlx::PgPool,
    setting_type: &str,
    department_id: Option<Uuid>,
) -> Result<serde_json::Value> {
    // 先查部门级，再查全局默认
    let row: Option<CodeToolSettingRow> = sqlx::query_as(
        "SELECT id, department_id, setting_type, config, updated_at \
         FROM code_tool_settings \
         WHERE setting_type = $1 AND department_id IS NOT DISTINCT FROM $2",
    )
    .bind(setting_type)
    .bind(department_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| Error::Database(e.to_string()))?;

    if let Some(row) = row {
        return Ok(json!({
            "id": row.id,
            "department_id": row.department_id,
            "setting_type": row.setting_type,
            "config": row.config,
            "updated_at": row.updated_at,
        }));
    }

    // 未找到部门级配置时，回退到全局
    if department_id.is_some() {
        let global: Option<CodeToolSettingRow> = sqlx::query_as(
            "SELECT id, department_id, setting_type, config, updated_at \
             FROM code_tool_settings \
             WHERE setting_type = $1 AND department_id IS NULL",
        )
        .bind(setting_type)
        .fetch_optional(pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        if let Some(row) = global {
            return Ok(json!({
                "id": row.id,
                "department_id": null,
                "setting_type": row.setting_type,
                "config": row.config,
                "updated_at": row.updated_at,
                "fallback": true,
            }));
        }
    }

    Err(Error::NotFound(format!(
        "Setting '{}' not found",
        setting_type
    )))
}

async fn upsert_setting(
    pool: &sqlx::PgPool,
    setting_type: &str,
    department_id: Option<Uuid>,
    config: &serde_json::Value,
    actor_id: Uuid,
) -> Result<serde_json::Value> {
    let row: CodeToolSettingRow = sqlx::query_as(
        "INSERT INTO code_tool_settings (department_id, setting_type, config, updated_by) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (department_id, setting_type) \
         DO UPDATE SET config = $3, updated_by = $4, updated_at = NOW() \
         RETURNING id, department_id, setting_type, config, updated_at",
    )
    .bind(department_id)
    .bind(setting_type)
    .bind(config)
    .bind(actor_id)
    .fetch_one(pool)
    .await
    .map_err(|e| Error::Database(e.to_string()))?;

    Ok(json!({
        "id": row.id,
        "department_id": row.department_id,
        "setting_type": row.setting_type,
        "config": row.config,
        "updated_at": row.updated_at,
    }))
}

async fn write_audit_sqlx(pool: &sqlx::PgPool, actor_id: Uuid, action: &str, details: &str) {
    let id = Uuid::new_v4();
    let now = chrono::Utc::now();
    let _ = sqlx::query(
        "INSERT INTO audit_logs (id, user_id, action, details, created_at, is_immutable) \
         VALUES ($1, $2, $3, $4, $5, FALSE)",
    )
    .bind(id)
    .bind(actor_id)
    .bind(action)
    .bind(details)
    .bind(now)
    .execute(pool)
    .await;
}

// ── Handlers ────────────────────────────────────────────────────────────

// ---------- code-tools ----------

#[instrument(skip(state))]
pub async fn get_code_tools(
    State(state): State<AppState>,
    Query(params): Query<DepartmentQuery>,
) -> Result<Json<serde_json::Value>> {
    let data = get_setting(&state.sqlx_pool, "code_tools", params.department_id).await?;
    Ok(Json(data))
}

#[instrument(skip(state, payload))]
pub async fn put_code_tools(
    State(state): State<AppState>,
    Extension(claims): Extension<TokenClaims>,
    Json(payload): Json<UpdateSettingPayload>,
) -> Result<Json<serde_json::Value>> {
    let actor_id = parse_actor(&claims)?;
    let data = upsert_setting(
        &state.sqlx_pool,
        "code_tools",
        payload.department_id,
        &payload.config,
        actor_id,
    )
    .await?;

    info!(?actor_id, dept = ?payload.department_id, "Updated code_tools settings");
    write_audit_sqlx(
        &state.sqlx_pool,
        actor_id,
        "update_code_tools",
        &format!("dept={:?}", payload.department_id),
    )
    .await;

    Ok(Json(data))
}

// ---------- workspace-paths ----------

#[instrument(skip(state))]
pub async fn get_workspace_paths(
    State(state): State<AppState>,
    Query(params): Query<DepartmentQuery>,
) -> Result<Json<serde_json::Value>> {
    let data = get_setting(&state.sqlx_pool, "workspace_paths", params.department_id).await?;
    Ok(Json(data))
}

#[instrument(skip(state, payload))]
pub async fn put_workspace_paths(
    State(state): State<AppState>,
    Extension(claims): Extension<TokenClaims>,
    Json(payload): Json<UpdateSettingPayload>,
) -> Result<Json<serde_json::Value>> {
    let actor_id = parse_actor(&claims)?;
    let data = upsert_setting(
        &state.sqlx_pool,
        "workspace_paths",
        payload.department_id,
        &payload.config,
        actor_id,
    )
    .await?;

    info!(?actor_id, dept = ?payload.department_id, "Updated workspace_paths settings");
    write_audit_sqlx(
        &state.sqlx_pool,
        actor_id,
        "update_workspace_paths",
        &format!("dept={:?}", payload.department_id),
    )
    .await;

    Ok(Json(data))
}

// ---------- bash-rules ----------

#[instrument(skip(state))]
pub async fn get_bash_rules(
    State(state): State<AppState>,
    Query(params): Query<DepartmentQuery>,
) -> Result<Json<serde_json::Value>> {
    let data = get_setting(&state.sqlx_pool, "bash_rules", params.department_id).await?;
    Ok(Json(data))
}

#[instrument(skip(state, payload))]
pub async fn put_bash_rules(
    State(state): State<AppState>,
    Extension(claims): Extension<TokenClaims>,
    Json(payload): Json<UpdateSettingPayload>,
) -> Result<Json<serde_json::Value>> {
    let actor_id = parse_actor(&claims)?;
    let data = upsert_setting(
        &state.sqlx_pool,
        "bash_rules",
        payload.department_id,
        &payload.config,
        actor_id,
    )
    .await?;

    info!(?actor_id, dept = ?payload.department_id, "Updated bash_rules settings");
    write_audit_sqlx(
        &state.sqlx_pool,
        actor_id,
        "update_bash_rules",
        &format!("dept={:?}", payload.department_id),
    )
    .await;

    Ok(Json(data))
}

// ── Private helpers ─────────────────────────────────────────────────────

// ---------- lsp-servers ----------

#[instrument(skip(state))]
pub async fn get_lsp_servers(
    State(state): State<AppState>,
    Query(params): Query<DepartmentQuery>,
) -> Result<Json<serde_json::Value>> {
    let data = get_setting(&state.sqlx_pool, "lsp_servers", params.department_id).await?;
    Ok(Json(data))
}

#[instrument(skip(state, payload))]
pub async fn put_lsp_servers(
    State(state): State<AppState>,
    Extension(claims): Extension<TokenClaims>,
    Json(payload): Json<UpdateSettingPayload>,
) -> Result<Json<serde_json::Value>> {
    let actor_id = parse_actor(&claims)?;
    let data = upsert_setting(
        &state.sqlx_pool,
        "lsp_servers",
        payload.department_id,
        &payload.config,
        actor_id,
    )
    .await?;

    info!(?actor_id, dept = ?payload.department_id, "Updated lsp_servers settings");
    write_audit_sqlx(
        &state.sqlx_pool,
        actor_id,
        "update_lsp_servers",
        &format!("dept={:?}", payload.department_id),
    )
    .await;

    Ok(Json(data))
}

// ---------- git-repos ----------

#[instrument(skip(state))]
pub async fn get_git_repos(
    State(state): State<AppState>,
    Query(params): Query<DepartmentQuery>,
) -> Result<Json<serde_json::Value>> {
    let data = get_setting(&state.sqlx_pool, "git_repos", params.department_id).await?;
    Ok(Json(data))
}

#[instrument(skip(state, payload))]
pub async fn put_git_repos(
    State(state): State<AppState>,
    Extension(claims): Extension<TokenClaims>,
    Json(payload): Json<UpdateSettingPayload>,
) -> Result<Json<serde_json::Value>> {
    let actor_id = parse_actor(&claims)?;
    let data = upsert_setting(
        &state.sqlx_pool,
        "git_repos",
        payload.department_id,
        &payload.config,
        actor_id,
    )
    .await?;

    info!(?actor_id, dept = ?payload.department_id, "Updated git_repos settings");
    write_audit_sqlx(
        &state.sqlx_pool,
        actor_id,
        "update_git_repos",
        &format!("dept={:?}", payload.department_id),
    )
    .await;

    Ok(Json(data))
}

// ---------- code-operations report ----------

#[derive(Debug, Deserialize)]
pub struct CodeOperationsQuery {
    pub department_id: Option<Uuid>,
    pub days: Option<i32>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct CodeOperationRow {
    action: String,
    total: i64,
}

#[instrument(skip(state))]
pub async fn get_code_operations(
    State(state): State<AppState>,
    Query(params): Query<CodeOperationsQuery>,
) -> Result<Json<serde_json::Value>> {
    let days = params.days.unwrap_or(7).min(90).max(1);

    let rows: Vec<CodeOperationRow> = sqlx::query_as(
        "SELECT action, COUNT(*) AS total \
         FROM audit_logs \
         WHERE action LIKE 'update_code_%' OR action LIKE 'update_lsp_%' OR action LIKE 'update_git_%' \
           AND created_at >= NOW() - make_interval(days => $1) \
         GROUP BY action \
         ORDER BY total DESC",
    )
    .bind(days)
    .fetch_all(&state.sqlx_pool)
    .await
    .map_err(|e| Error::Database(e.to_string()))?;

    Ok(Json(json!({
        "period_days": days,
        "department_id": params.department_id,
        "operations": rows,
    })))
}

// ── Private helpers ─────────────────────────────────────────────────────

fn parse_actor(claims: &TokenClaims) -> Result<Uuid> {
    Uuid::parse_str(&claims.sub).map_err(|_| Error::Unauthorized)
}
