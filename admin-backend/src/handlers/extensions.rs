//! 扩展管理 Handler（需求 14 重构）
//!
//! 职责：
//! - 技能/插件的 CRUD（直查 Admin DB，不再代理 Gateway）
//! - 上传 + 安全扫描 + 审核流程
//! - 私有注册表 API（兼容 ClawHub `/api/v1/` 格式，供 ironclaw 引擎使用）
//! - 部门技能白名单管理

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::{
    error::{Error, Result},
    routes::write_audit_log,
    AppState,
};

type DbPool = deadpool_postgres::Pool;

// ── 数据类型 ─────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct SkillRow {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub version: String,
    pub author: String,
    pub enabled: bool,
    pub source: String,
    pub review_status: String,
    pub is_builtin: bool,
    pub invoke_count: i64,
    pub file_path: Option<String>,
    pub review_note: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct PluginRow {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub version: String,
    pub author: String,
    pub enabled: bool,
    pub source: String,
    pub review_status: String,
    pub plugin_type: String,
    pub is_builtin: bool,
    pub invoke_count: i64,
    pub requires_sandbox: bool,
    pub review_note: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct ReviewRequest {
    pub approved: bool,
    pub note: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RegistrySearchQuery {
    pub q: Option<String>,
    /// client_token 用于识别用户所属部门，按白名单过滤结果
    pub client_token: Option<String>,
    /// user_id 备用身份标识
    pub user_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RegistryDownloadQuery {
    pub slug: String,
    pub client_token: Option<String>,
}

// ── 技能列表（重写：直查 Admin DB）────────────────────────────────────────────

pub async fn list_skills(State(state): State<AppState>) -> Result<Json<serde_json::Value>> {
    let skills = sqlx::query_as::<_, SkillRow>(
        "SELECT id, name, COALESCE(description,'') AS description,
                version, COALESCE(author,'') AS author, enabled,
                source, review_status, is_builtin, invoke_count,
                file_path, review_note, created_at, updated_at
         FROM skills ORDER BY name ASC",
    )
    .fetch_all(&state.sqlx_pool)
    .await
    .map_err(|e| Error::Database(e.to_string()))?;

    Ok(Json(json!({ "skills": skills, "count": skills.len() })))
}

// ── 插件列表（重写：直查 Admin DB）────────────────────────────────────────────

pub async fn list_plugins(State(state): State<AppState>) -> Result<Json<serde_json::Value>> {
    let plugins = sqlx::query_as::<_, PluginRow>(
        "SELECT id, name, COALESCE(description,'') AS description,
                version, COALESCE(author,'') AS author, enabled,
                source, review_status, plugin_type, is_builtin,
                invoke_count, requires_sandbox, review_note,
                created_at, updated_at
         FROM plugins ORDER BY name ASC",
    )
    .fetch_all(&state.sqlx_pool)
    .await
    .map_err(|e| Error::Database(e.to_string()))?;

    Ok(Json(json!({ "plugins": plugins, "count": plugins.len() })))
}

// ── 启用/禁用（技能和插件共用逻辑）─────────────────────────────────────────────

async fn set_item_enabled(
    sqlx_pool: &sqlx::PgPool,
    db_pool: &DbPool,
    table: &str,
    item_id: Uuid,
    action: &str,
) -> Result<Json<serde_json::Value>> {
    let enabled = match action {
        "enable" => true,
        "disable" => false,
        _ => return Err(Error::Validation("action 必须为 enable 或 disable".into())),
    };

    // table 来自内部调用，只能是 "skills" 或 "plugins"，无 SQL 注入风险
    let sql = format!(
        "UPDATE {} SET enabled = $1, updated_at = NOW() WHERE id = $2",
        table
    );
    let rows = sqlx::query(&sql)
        .bind(enabled)
        .bind(item_id)
        .execute(sqlx_pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .rows_affected();

    if rows == 0 {
        return Err(Error::NotFound(format!(
            "{}不存在",
            if table == "skills" {
                "技能"
            } else {
                "插件"
            }
        )));
    }

    let db = db_pool
        .get()
        .await
        .map_err(|e| Error::Database(e.to_string()))?;
    write_audit_log(
        &db,
        Uuid::nil(),
        &format!("{}_toggle", table),
        &format!(
            "{} {} 已{}",
            table,
            item_id,
            if enabled { "启用" } else { "禁用" }
        ),
    )
    .await;

    Ok(Json(json!({ "id": item_id, "enabled": enabled })))
}

pub async fn set_skill_enabled(
    State(state): State<AppState>,
    Path((skill_id, action)): Path<(Uuid, String)>,
) -> Result<Json<serde_json::Value>> {
    set_item_enabled(
        &state.sqlx_pool,
        &state.db_pool,
        "skills",
        skill_id,
        &action,
    )
    .await
}

pub async fn set_plugin_enabled(
    State(state): State<AppState>,
    Path((plugin_id, action)): Path<(Uuid, String)>,
) -> Result<Json<serde_json::Value>> {
    set_item_enabled(
        &state.sqlx_pool,
        &state.db_pool,
        "plugins",
        plugin_id,
        &action,
    )
    .await
}

// ── 技能上传（格式校验 + 安全扫描）──────────────────────────────────────────────

/// 提示词注入关键词列表（Fail-Safe：命中则拒绝）
const INJECTION_KEYWORDS: &[&str] = &[
    "ignore previous instructions",
    "ignore all previous",
    "disregard your instructions",
    "you are now",
    "act as",
    "jailbreak",
    "dan mode",
    "developer mode",
];

fn scan_for_injection(content: &str) -> Option<&'static str> {
    let lower = content.to_lowercase();
    INJECTION_KEYWORDS
        .iter()
        .find(|kw| lower.contains(*kw))
        .copied()
}

#[derive(Debug, Deserialize)]
pub struct SkillUploadRequest {
    pub name: String,
    pub content: String, // SKILL.md 文本内容
    pub version: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
}

pub async fn upload_skill(
    State(state): State<AppState>,
    Json(payload): Json<SkillUploadRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>)> {
    // 格式校验：必须包含 YAML frontmatter
    if !payload.content.starts_with("---") {
        return Err(Error::Validation(
            "技能包格式错误：缺少 YAML frontmatter（以 --- 开头）".into(),
        ));
    }

    // 安全扫描：Fail-Safe，命中则拒绝
    if let Some(kw) = scan_for_injection(&payload.content) {
        return Err(Error::Validation(format!(
            "安全扫描未通过：检测到提示词注入关键词「{}」",
            kw
        )));
    }

    let skill_id = Uuid::new_v4();
    let version = payload.version.as_deref().unwrap_or("1.0.0");
    let description = payload.description.as_deref().unwrap_or("");
    let author = payload.author.as_deref().unwrap_or("");

    sqlx::query(
        "INSERT INTO skills (id, name, description, version, author, enabled,
                             source, review_status, is_builtin, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, false, 'admin_upload', 'pending', false, NOW(), NOW())",
    )
    .bind(skill_id)
    .bind(&payload.name)
    .bind(description)
    .bind(version)
    .bind(author)
    .execute(&state.sqlx_pool)
    .await
    .map_err(|e| Error::Database(e.to_string()))?;

    let db = state
        .db_pool
        .get()
        .await
        .map_err(|e| Error::Database(e.to_string()))?;
    write_audit_log(
        &db,
        Uuid::nil(),
        "skill_upload",
        &format!("上传技能包：{} ({})", payload.name, skill_id),
    )
    .await;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": skill_id,
            "name": payload.name,
            "review_status": "pending",
            "message": "技能包已上传，等待审核"
        })),
    ))
}

// ── 审核（技能和插件共用逻辑）────────────────────────────────────────────────

async fn review_item(
    sqlx_pool: &sqlx::PgPool,
    db_pool: &DbPool,
    table: &str,
    item_id: Uuid,
    payload: ReviewRequest,
) -> Result<Json<serde_json::Value>> {
    let (new_status, enabled) = if payload.approved {
        ("approved", true)
    } else {
        ("rejected", false)
    };

    let sql = format!(
        "UPDATE {} SET review_status = $1, enabled = $2, review_note = $3,
             reviewed_at = NOW(), updated_at = NOW()
         WHERE id = $4 AND review_status = 'pending'",
        table
    );
    let rows = sqlx::query(&sql)
        .bind(new_status)
        .bind(enabled)
        .bind(payload.note.as_deref())
        .bind(item_id)
        .execute(sqlx_pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .rows_affected();

    if rows == 0 {
        return Err(Error::NotFound(format!(
            "{}不存在或已审核",
            if table == "skills" {
                "技能"
            } else {
                "插件"
            }
        )));
    }

    let db = db_pool
        .get()
        .await
        .map_err(|e| Error::Database(e.to_string()))?;
    write_audit_log(
        &db,
        Uuid::nil(),
        &format!("{}_review", table),
        &format!(
            "{} {} 审核{}：{}",
            table,
            item_id,
            if payload.approved { "通过" } else { "拒绝" },
            payload.note.as_deref().unwrap_or("-")
        ),
    )
    .await;

    Ok(Json(
        json!({ "id": item_id, "review_status": new_status, "enabled": enabled }),
    ))
}

// ── 技能审核 ──────────────────────────────────────────────────────────────────

pub async fn review_skill(
    State(state): State<AppState>,
    Path(skill_id): Path<Uuid>,
    Json(payload): Json<ReviewRequest>,
) -> Result<Json<serde_json::Value>> {
    review_item(
        &state.sqlx_pool,
        &state.db_pool,
        "skills",
        skill_id,
        payload,
    )
    .await
}

// ── 插件审核 ──────────────────────────────────────────────────────────────────

pub async fn review_plugin(
    State(state): State<AppState>,
    Path(plugin_id): Path<Uuid>,
    Json(payload): Json<ReviewRequest>,
) -> Result<Json<serde_json::Value>> {
    review_item(
        &state.sqlx_pool,
        &state.db_pool,
        "plugins",
        plugin_id,
        payload,
    )
    .await
}

// ── 插件上传 ──────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct PluginUploadRequest {
    pub name: String,
    pub plugin_type: String, // "http" | "stdio" | "wasm"
    pub version: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
}

pub async fn upload_plugin(
    State(state): State<AppState>,
    Json(payload): Json<PluginUploadRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>)> {
    let plugin_type = payload.plugin_type.as_str();
    if !matches!(plugin_type, "http" | "stdio" | "wasm") {
        return Err(Error::Validation(
            "plugin_type 必须为 http、stdio 或 wasm".into(),
        ));
    }

    // Stdio 类型自动标记需要沙箱
    let requires_sandbox = plugin_type == "stdio";

    let plugin_id = Uuid::new_v4();
    let version = payload.version.as_deref().unwrap_or("1.0.0");
    let description = payload.description.as_deref().unwrap_or("");
    let author = payload.author.as_deref().unwrap_or("");

    sqlx::query(
        "INSERT INTO plugins (id, name, description, version, author, enabled,
                              source, review_status, plugin_type, is_builtin,
                              requires_sandbox, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, false, 'admin_upload', 'pending', $6, false, $7, NOW(), NOW())",
    )
    .bind(plugin_id)
    .bind(&payload.name)
    .bind(description)
    .bind(version)
    .bind(author)
    .bind(plugin_type)
    .bind(requires_sandbox)
    .execute(&state.sqlx_pool)
    .await
    .map_err(|e| Error::Database(e.to_string()))?;

    let db = state
        .db_pool
        .get()
        .await
        .map_err(|e| Error::Database(e.to_string()))?;
    write_audit_log(
        &db,
        Uuid::nil(),
        "plugin_upload",
        &format!(
            "上传插件包：{} ({}) 类型={}",
            payload.name, plugin_id, plugin_type
        ),
    )
    .await;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": plugin_id,
            "name": payload.name,
            "plugin_type": plugin_type,
            "requires_sandbox": requires_sandbox,
            "review_status": "pending",
            "message": "插件包已上传，等待审核"
        })),
    ))
}

// ── 私有注册表 API（兼容 ClawHub /api/v1/ 格式）────────────────────────────────
//
// ironclaw 引擎调用：
//   GET {registry_url}/api/v1/search?q=xxx&client_token=yyy
//   GET {registry_url}/api/v1/download?slug=xxx&client_token=yyy
//   GET {registry_url}/api/v1/skills/{slug}
//
// client_token 用于查找用户所属部门，按白名单过滤结果。
// 未提供 token 或 token 无效时，返回所有已审核通过且已启用的技能。

/// 从 client_token 解析出部门 ID（查 registered_clients 表）
async fn resolve_department_id(pool: &sqlx::PgPool, client_token: Option<&str>) -> Option<Uuid> {
    let token = client_token?;

    #[derive(sqlx::FromRow)]
    struct Row {
        department_id: Option<Uuid>,
    }

    sqlx::query_as::<_, Row>(
        "SELECT u.department_id
         FROM registered_clients rc
         JOIN users u ON rc.user_id = u.id
         WHERE rc.client_token = $1
         LIMIT 1",
    )
    .bind(token)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .and_then(|r| r.department_id)
}

/// GET /api/v1/search?q=xxx&client_token=yyy
pub async fn registry_search(
    State(state): State<AppState>,
    Query(params): Query<RegistrySearchQuery>,
) -> Result<Json<serde_json::Value>> {
    let query = params.q.as_deref().unwrap_or("").to_lowercase();
    let token = params.client_token.as_deref().or(params.user_id.as_deref());
    let dept_id = resolve_department_id(&state.sqlx_pool, token).await;

    let skills = match dept_id {
        Some(did) => fetch_skills_for_department(&state.sqlx_pool, did, &query).await?,
        None => fetch_all_enabled_skills(&state.sqlx_pool, &query).await?,
    };

    Ok(Json(json!({ "results": skills })))
}

/// 按部门查询：有白名单则过滤，无白名单则返回全部
async fn fetch_skills_for_department(
    pool: &sqlx::PgPool,
    dept_id: Uuid,
    query: &str,
) -> Result<Vec<serde_json::Value>> {
    #[derive(sqlx::FromRow)]
    struct CountRow {
        count: i64,
    }

    let cnt = sqlx::query_as::<_, CountRow>(
        "SELECT COUNT(*) AS count FROM department_skill_whitelist WHERE department_id = $1",
    )
    .bind(dept_id)
    .fetch_one(pool)
    .await
    .map_err(|e| Error::Database(e.to_string()))?;

    if cnt.count > 0 {
        fetch_skills_by_whitelist(pool, dept_id, query).await
    } else {
        fetch_all_enabled_skills(pool, query).await
    }
}

#[derive(sqlx::FromRow)]
struct SkillSearchRow {
    id: Uuid,
    name: String,
    description: String,
    version: String,
}

fn skill_to_registry_entry(s: &SkillSearchRow) -> serde_json::Value {
    json!({
        "slug": s.id.to_string(),
        "display_name": s.name,
        "summary": s.description,
        "version": s.version,
        "score": 1.0,
    })
}

async fn fetch_all_enabled_skills(
    pool: &sqlx::PgPool,
    query: &str,
) -> Result<Vec<serde_json::Value>> {
    let rows = sqlx::query_as::<_, SkillSearchRow>(
        "SELECT id, name, COALESCE(description,'') AS description, version
         FROM skills
         WHERE enabled = true AND review_status = 'approved'
           AND ($1 = '' OR LOWER(name) LIKE $2 OR LOWER(description) LIKE $2)
         ORDER BY name ASC
         LIMIT 50",
    )
    .bind(query)
    .bind(format!("%{}%", query))
    .fetch_all(pool)
    .await
    .map_err(|e| Error::Database(e.to_string()))?;

    Ok(rows.iter().map(skill_to_registry_entry).collect())
}

async fn fetch_skills_by_whitelist(
    pool: &sqlx::PgPool,
    dept_id: Uuid,
    query: &str,
) -> Result<Vec<serde_json::Value>> {
    let rows = sqlx::query_as::<_, SkillSearchRow>(
        "SELECT s.id, s.name, COALESCE(s.description,'') AS description, s.version
         FROM skills s
         JOIN department_skill_whitelist w ON s.id = w.skill_id
         WHERE w.department_id = $1
           AND s.enabled = true AND s.review_status = 'approved'
           AND ($2 = '' OR LOWER(s.name) LIKE $3 OR LOWER(s.description) LIKE $3)
         ORDER BY s.name ASC
         LIMIT 50",
    )
    .bind(dept_id)
    .bind(query)
    .bind(format!("%{}%", query))
    .fetch_all(pool)
    .await
    .map_err(|e| Error::Database(e.to_string()))?;

    Ok(rows.iter().map(skill_to_registry_entry).collect())
}

/// GET /api/v1/download?slug=xxx&client_token=yyy
///
/// 返回 SKILL.md 文本内容（纯文本）。
/// slug 即技能 UUID，file_path 存储 SKILL.md 内容。
pub async fn registry_download(
    State(state): State<AppState>,
    Query(params): Query<RegistryDownloadQuery>,
) -> Result<axum::response::Response<String>> {
    #[derive(sqlx::FromRow)]
    struct Row {
        file_path: Option<String>,
        enabled: bool,
        review_status: String,
    }

    let skill_id =
        Uuid::parse_str(&params.slug).map_err(|_| Error::Validation("slug 格式无效".into()))?;

    let row = sqlx::query_as::<_, Row>(
        "SELECT file_path, enabled, review_status FROM skills WHERE id = $1",
    )
    .bind(skill_id)
    .fetch_optional(&state.sqlx_pool)
    .await
    .map_err(|e| Error::Database(e.to_string()))?
    .ok_or_else(|| Error::NotFound("技能不存在".into()))?;

    if !row.enabled || row.review_status != "approved" {
        return Err(Error::NotFound("技能不可用".into()));
    }

    let content = row.file_path.unwrap_or_default();

    Ok(axum::response::Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/plain; charset=utf-8")
        .body(content)
        .expect("response builder should not fail"))
}

/// GET /api/v1/skills/{slug}
///
/// 返回技能详情（兼容 ClawHub SkillDetailResponse 格式）。
pub async fn registry_skill_detail(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Json<serde_json::Value>> {
    let skill_id = Uuid::parse_str(&slug).map_err(|_| Error::Validation("slug 格式无效".into()))?;

    #[derive(sqlx::FromRow)]
    struct Row {
        name: String,
        description: String,
        version: String,
        invoke_count: i64,
    }

    let row = sqlx::query_as::<_, Row>(
        "SELECT name, COALESCE(description,'') AS description, version, invoke_count
         FROM skills WHERE id = $1 AND enabled = true AND review_status = 'approved'",
    )
    .bind(skill_id)
    .fetch_optional(&state.sqlx_pool)
    .await
    .map_err(|e| Error::Database(e.to_string()))?
    .ok_or_else(|| Error::NotFound("技能不存在".into()))?;

    // 兼容 ClawHub SkillDetailResponse 格式
    Ok(Json(json!({
        "skill": {
            "slug": slug,
            "display_name": row.name,
            "summary": row.description,
            "updated_at": null,
        },
        "owner": null,
        "stats": {
            "installs_current": row.invoke_count,
            "downloads": row.invoke_count,
            "stars": null,
        }
    })))
}

// ── 部门技能白名单 ─────────────────────────────────────────────────────────────

pub async fn get_skill_whitelist(
    State(state): State<AppState>,
    Path(dept_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    #[derive(sqlx::FromRow, Serialize)]
    struct Row {
        skill_id: Uuid,
        name: String,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT s.id AS skill_id, s.name
         FROM department_skill_whitelist w
         JOIN skills s ON s.id = w.skill_id
         WHERE w.department_id = $1
         ORDER BY s.name ASC",
    )
    .bind(dept_id)
    .fetch_all(&state.sqlx_pool)
    .await
    .map_err(|e| Error::Database(e.to_string()))?;

    Ok(Json(
        json!({ "skill_ids": rows.iter().map(|r| r.skill_id).collect::<Vec<_>>(), "skills": rows }),
    ))
}

#[derive(Debug, Deserialize)]
pub struct UpdateSkillWhitelistRequest {
    pub skill_ids: Vec<Uuid>,
}

pub async fn update_skill_whitelist(
    State(state): State<AppState>,
    Path(dept_id): Path<Uuid>,
    Json(payload): Json<UpdateSkillWhitelistRequest>,
) -> Result<Json<serde_json::Value>> {
    // 验证部门存在
    let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM departments WHERE id = $1")
        .bind(dept_id)
        .fetch_one(&state.sqlx_pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    if exists == 0 {
        return Err(Error::NotFound("部门不存在".into()));
    }

    // 全量替换：先删后插
    let mut tx = state
        .sqlx_pool
        .begin()
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    sqlx::query("DELETE FROM department_skill_whitelist WHERE department_id = $1")
        .bind(dept_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    for skill_id in &payload.skill_ids {
        sqlx::query(
            "INSERT INTO department_skill_whitelist (department_id, skill_id)
             VALUES ($1, $2) ON CONFLICT DO NOTHING",
        )
        .bind(dept_id)
        .bind(skill_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;
    }

    tx.commit()
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let db = state
        .db_pool
        .get()
        .await
        .map_err(|e| Error::Database(e.to_string()))?;
    write_audit_log(
        &db,
        Uuid::nil(),
        "dept_skill_whitelist_update",
        &format!(
            "部门 {} 技能白名单更新：{} 条",
            dept_id,
            payload.skill_ids.len()
        ),
    )
    .await;

    Ok(Json(
        json!({ "department_id": dept_id, "count": payload.skill_ids.len() }),
    ))
}
