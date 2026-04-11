//! 扩展管理 Handler（需求 14 重构）
//!
//! 职责：
//! - 技能/插件的 CRUD（直查 Admin DB，不再代理 Gateway）
//! - 上传 + 安全扫描 + 审核流程
//! - 私有注册表 API（兼容 ClawHub `/api/v1/` 格式，供 ironclaw 引擎使用）
//! - 部门技能白名单管理

use axum::{
    extract::{Multipart, Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::{
    error::{Error, Result},
    extensions_state::{
        enabled_after_rescan, review_status_after_rescan, REVIEW_STATUS_PENDING,
        REVIEW_STATUS_SCANNING,
        REVIEW_STATUS_SCAN_FAILED, REVIEW_STATUS_YANKED,
    },
    extensions_validation::{extract_skill_metadata, validate_skill_package},
    routes::write_audit_log,
    scanner::{FindingSeverity, ScanResult, ScannerConfig, SkillScanner},
    skill_package::extract_skill_content,
    AppState,
};

type DbPool = deadpool_postgres::Pool;
const MAX_UPLOAD_PACKAGE_BYTES: usize = 8 * 1024 * 1024;

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
    pub reviewed_by: Option<Uuid>,
    pub reviewed_at: Option<chrono::DateTime<chrono::Utc>>,
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
    pub reviewed_by: Option<Uuid>,
    pub reviewed_at: Option<chrono::DateTime<chrono::Utc>>,
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
                file_path, review_note, reviewed_by, reviewed_at, created_at, updated_at
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
                reviewed_by, reviewed_at, created_at, updated_at
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

// ── 技能上传（格式校验 + 扫描流程）──────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct SkillUploadRequest {
    pub name: String,
    pub content: String, // SKILL.md 文本内容
    pub version: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
}

struct UploadedSkillPackage {
    file_name: String,
    content: String,
}

fn severity_to_string(severity: FindingSeverity) -> &'static str {
    match severity {
        FindingSeverity::Low => "LOW",
        FindingSeverity::Medium => "MEDIUM",
        FindingSeverity::High => "HIGH",
        FindingSeverity::Critical => "CRITICAL",
        FindingSeverity::Unknown => "UNKNOWN",
    }
}

async fn save_scan_result(sqlx_pool: &sqlx::PgPool, skill_id: Uuid, scan: &ScanResult) -> Result<()> {
    let findings = serde_json::to_value(&scan.findings)
        .map_err(|e| Error::Internal(format!("序列化扫描结果失败: {}", e)))?;
    let verdict = format!("{:?}", scan.verdict).to_uppercase();
    let max_severity = scan.max_severity.map(severity_to_string);

    sqlx::query(
        "INSERT INTO scan_results (
            target_type, target_id, scanner_type, verdict, is_safe,
            max_severity, findings_count, findings, scan_duration_ms, scanned_at
         ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NOW())",
    )
    .bind("skill")
    .bind(skill_id)
    .bind(&scan.scanner_type)
    .bind(verdict)
    .bind(scan.is_safe)
    .bind(max_severity)
    .bind(scan.findings_count)
    .bind(findings)
    .bind(scan.scan_duration_ms)
    .execute(sqlx_pool)
    .await
    .map_err(|e| Error::Database(e.to_string()))?;

    Ok(())
}

async fn update_skill_review_status(
    sqlx_pool: &sqlx::PgPool,
    skill_id: Uuid,
    review_status: &str,
) -> Result<String> {
    sqlx::query(
        "UPDATE skills
         SET review_status = $1, updated_at = NOW()
         WHERE id = $2",
    )
    .bind(review_status)
    .bind(skill_id)
    .execute(sqlx_pool)
    .await
    .map_err(|e| Error::Database(e.to_string()))?;

    Ok(review_status.to_string())
}

async fn run_skill_scan_and_persist(
    sqlx_pool: &sqlx::PgPool,
    skill_id: Uuid,
    skill_name: &str,
    content: &str,
    scanner_cfg: &ScannerConfig,
) -> Result<()> {
    let scanner = SkillScanner::new(&scanner_cfg.url, scanner_cfg.timeout_ms)
        .map_err(|e| Error::Internal(format!("初始化扫描器失败: {}", e)))?;

    match scanner.scan_upload(skill_name, content).await {
        Ok(scan_result) => {
            save_scan_result(sqlx_pool, skill_id, &scan_result).await?;
            let next_status = if scan_result.is_safe {
                REVIEW_STATUS_PENDING
            } else {
                REVIEW_STATUS_SCAN_FAILED
            };
            let _ = update_skill_review_status(sqlx_pool, skill_id, next_status).await?;
        }
        Err(error) => {
            tracing::warn!(
                skill_id = %skill_id,
                skill = %skill_name,
                error = %error,
                "Security scan request failed"
            );
            let _ = update_skill_review_status(sqlx_pool, skill_id, REVIEW_STATUS_SCAN_FAILED).await?;
        }
    }

    Ok(())
}

fn spawn_skill_scan_job(
    sqlx_pool: sqlx::PgPool,
    skill_id: Uuid,
    skill_name: String,
    content: String,
    scanner_cfg: ScannerConfig,
) {
    tokio::spawn(async move {
        if let Err(error) = run_skill_scan_and_persist(
            &sqlx_pool,
            skill_id,
            &skill_name,
            &content,
            &scanner_cfg,
        )
        .await
        {
            tracing::error!(
                skill_id = %skill_id,
                skill = %skill_name,
                error = %error,
                "Background skill scan job failed"
            );
            if let Err(update_error) =
                update_skill_review_status(&sqlx_pool, skill_id, REVIEW_STATUS_SCAN_FAILED).await
            {
                tracing::error!(
                    skill_id = %skill_id,
                    error = %update_error,
                    "Failed to set skill review status to scan_failed after scan job failure"
                );
            }
        }
    });
}

pub async fn upload_skill(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<SkillUploadRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>)> {
    validate_skill_package(&payload.content)?;
    upload_skill_from_payload(&state, &headers, payload, "json").await
}

pub async fn upload_skill_package(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<serde_json::Value>)> {
    let uploaded = read_skill_package_from_multipart(&mut multipart).await?;
    validate_skill_package(&uploaded.content)?;

    let metadata = extract_skill_metadata(&uploaded.content)?;
    let payload = SkillUploadRequest {
        name: metadata.name,
        content: uploaded.content,
        version: metadata.version,
        description: metadata.description,
        author: metadata.author,
    };

    upload_skill_from_payload(&state, &headers, payload, &uploaded.file_name).await
}

async fn read_skill_package_from_multipart(
    multipart: &mut Multipart,
) -> Result<UploadedSkillPackage> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| Error::Validation(format!("读取上传字段失败: {}", e)))?
    {
        let Some(file_name) = field.file_name().map(ToString::to_string) else {
            continue;
        };

        let bytes = read_field_bytes_limited(field, MAX_UPLOAD_PACKAGE_BYTES).await?;
        let content = extract_skill_content(&file_name, &bytes)?;

        return Ok(UploadedSkillPackage { file_name, content });
    }

    Err(Error::Validation("未检测到上传文件字段".into()))
}

async fn read_field_bytes_limited(
    mut field: axum::extract::multipart::Field<'_>,
    max_bytes: usize,
) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();

    while let Some(chunk) = field
        .chunk()
        .await
        .map_err(|e| Error::Validation(format!("读取上传文件失败: {}", e)))?
    {
        if bytes.len() + chunk.len() > max_bytes {
            return Err(Error::Validation(format!(
                "上传包超过 {} MiB 限制",
                max_bytes / (1024 * 1024)
            )));
        }
        bytes.extend_from_slice(&chunk);
    }

    Ok(bytes)
}

async fn upload_skill_from_payload(
    state: &AppState,
    headers: &HeaderMap,
    payload: SkillUploadRequest,
    source_label: &str,
) -> Result<(StatusCode, Json<serde_json::Value>)> {

    let skill_id = Uuid::new_v4();
    let uploader_id = admin_user_id_from_headers(headers);
    let version = payload.version.as_deref().unwrap_or("1.0.0");
    let description = payload.description.as_deref().unwrap_or("");
    let author = payload.author.as_deref().unwrap_or("");
    let scanner_cfg = ScannerConfig::from_env();
    let review_status = if scanner_cfg.enabled {
        REVIEW_STATUS_SCANNING.to_string()
    } else {
        REVIEW_STATUS_PENDING.to_string()
    };

    sqlx::query(
           "INSERT INTO skills (id, name, description, version, author, uploaded_by, file_path,
                            enabled, source, review_status, is_builtin, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, false, 'admin_upload', $8, false, NOW(), NOW())",
    )
    .bind(skill_id)
    .bind(&payload.name)
    .bind(description)
    .bind(version)
    .bind(author)
        .bind(uploader_id)
    .bind(&payload.content)
    .bind(&review_status)
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
        &format!(
            "上传技能包：{} ({}) source={} ",
            payload.name, skill_id, source_label
        ),
    )
    .await;

    if scanner_cfg.enabled {
        spawn_skill_scan_job(
            state.sqlx_pool.clone(),
            skill_id,
            payload.name.clone(),
            payload.content.clone(),
            scanner_cfg,
        );
    }

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": skill_id,
            "name": payload.name,
            "review_status": review_status,
            "message": if review_status == REVIEW_STATUS_SCANNING {
                "技能包已上传，正在执行安全扫描"
            } else {
                "技能包已上传，等待审核"
            }
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
    reviewer_id: Option<Uuid>,
) -> Result<Json<serde_json::Value>> {
    let (new_status, enabled) = if payload.approved {
        ("approved", true)
    } else {
        ("rejected", false)
    };

    let sql = format!(
        "UPDATE {} SET review_status = $1, enabled = $2, review_note = $3,
             reviewed_by = $4, reviewed_at = NOW(), updated_at = NOW()
         WHERE id = $5 AND review_status = $6",
        table
    );
    let rows = sqlx::query(&sql)
        .bind(new_status)
        .bind(enabled)
        .bind(payload.note.as_deref())
        .bind(reviewer_id)
        .bind(item_id)
        .bind(REVIEW_STATUS_PENDING)
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
            "{} {} 审核{}：{} reviewer={} ",
            table,
            item_id,
            if payload.approved { "通过" } else { "拒绝" },
            payload.note.as_deref().unwrap_or("-"),
            reviewer_id
                .map(|id| id.to_string())
                .unwrap_or_else(|| "unknown".to_string())
        ),
    )
    .await;

    Ok(Json(
        json!({ "id": item_id, "review_status": new_status, "enabled": enabled }),
    ))
}

fn admin_user_id_from_headers(headers: &HeaderMap) -> Option<Uuid> {
    headers
        .get("x-admin-user-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| Uuid::parse_str(v).ok())
}

// ── 技能审核 ──────────────────────────────────────────────────────────────────

pub async fn review_skill(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(skill_id): Path<Uuid>,
    Json(payload): Json<ReviewRequest>,
) -> Result<Json<serde_json::Value>> {
    let reviewer_id = admin_user_id_from_headers(&headers);
    review_item(
        &state.sqlx_pool,
        &state.db_pool,
        "skills",
        skill_id,
        payload,
        reviewer_id,
    )
    .await
}

// ── 插件审核 ──────────────────────────────────────────────────────────────────

pub async fn review_plugin(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(plugin_id): Path<Uuid>,
    Json(payload): Json<ReviewRequest>,
) -> Result<Json<serde_json::Value>> {
    let reviewer_id = admin_user_id_from_headers(&headers);
    review_item(
        &state.sqlx_pool,
        &state.db_pool,
        "plugins",
        plugin_id,
        payload,
        reviewer_id,
    )
    .await
}

#[derive(Debug, Deserialize)]
pub struct YankSkillRequest {
    pub note: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct YankPluginRequest {
    pub note: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct SkillStateRow {
    name: String,
    file_path: Option<String>,
    review_status: String,
    enabled: bool,
}

#[derive(Debug, sqlx::FromRow)]
struct PluginStateRow {
    review_status: String,
}

fn scanner_disabled_error() -> Error {
    Error::Validation("安全扫描器未启用，无法执行重扫".into())
}

async fn fetch_skill_state(sqlx_pool: &sqlx::PgPool, skill_id: Uuid) -> Result<SkillStateRow> {
    let row = sqlx::query_as::<_, SkillStateRow>(
        "SELECT name, file_path, review_status, enabled
         FROM skills
         WHERE id = $1",
    )
    .bind(skill_id)
    .fetch_optional(sqlx_pool)
    .await
    .map_err(|e| Error::Database(e.to_string()))?
    .ok_or_else(|| Error::NotFound("技能不存在".into()))?;

    Ok(row)
}

async fn fetch_plugin_state(sqlx_pool: &sqlx::PgPool, plugin_id: Uuid) -> Result<PluginStateRow> {
    let row = sqlx::query_as::<_, PluginStateRow>(
        "SELECT review_status
         FROM plugins
         WHERE id = $1",
    )
    .bind(plugin_id)
    .fetch_optional(sqlx_pool)
    .await
    .map_err(|e| Error::Database(e.to_string()))?
    .ok_or_else(|| Error::NotFound("插件不存在".into()))?;

    Ok(row)
}

async fn update_skill_state_after_rescan(
    sqlx_pool: &sqlx::PgPool,
    skill_id: Uuid,
    review_status: &str,
    enabled: bool,
) -> Result<()> {
    sqlx::query(
        "UPDATE skills
         SET review_status = $1, enabled = $2, updated_at = NOW()
         WHERE id = $3",
    )
    .bind(review_status)
    .bind(enabled)
    .bind(skill_id)
    .execute(sqlx_pool)
    .await
    .map_err(|e| Error::Database(e.to_string()))?;

    Ok(())
}

async fn persist_rescan_result(
    sqlx_pool: &sqlx::PgPool,
    skill_id: Uuid,
    current_status: &str,
    current_enabled: bool,
    scan_result: &ScanResult,
) -> Result<(String, bool)> {
    save_scan_result(sqlx_pool, skill_id, scan_result).await?;

    let next_status = review_status_after_rescan(current_status, scan_result.is_safe).to_string();
    let next_enabled = enabled_after_rescan(current_enabled, current_status, scan_result.is_safe);
    update_skill_state_after_rescan(sqlx_pool, skill_id, &next_status, next_enabled).await?;

    Ok((next_status, next_enabled))
}

async fn run_skill_rescan(
    sqlx_pool: &sqlx::PgPool,
    scanner: &SkillScanner,
    skill_id: Uuid,
    skill_name: &str,
    content: &str,
    current_status: &str,
    current_enabled: bool,
) -> Result<(String, bool, bool, i32)> {
    match scanner.scan_upload(skill_name, content).await {
        Ok(scan_result) => {
            let (next_status, next_enabled) = persist_rescan_result(
                sqlx_pool,
                skill_id,
                current_status,
                current_enabled,
                &scan_result,
            )
            .await?;

            Ok((
                next_status,
                next_enabled,
                scan_result.is_safe,
                scan_result.findings_count,
            ))
        }
        Err(_) => {
            let next_status = REVIEW_STATUS_SCAN_FAILED.to_string();
            update_skill_state_after_rescan(sqlx_pool, skill_id, &next_status, false).await?;
            Ok((next_status, false, false, 0))
        }
    }
}

async fn write_skill_rescan_audit(
    db_pool: &DbPool,
    skill_id: Uuid,
    previous_status: &str,
    next_status: &str,
    is_safe: bool,
) -> Result<()> {
    let db = db_pool
        .get()
        .await
        .map_err(|e| Error::Database(e.to_string()))?;
    write_audit_log(
        &db,
        Uuid::nil(),
        "skill_rescan",
        &format!(
            "技能 {} 重扫完成: {} -> {} (safe={})",
            skill_id, previous_status, next_status, is_safe
        ),
    )
    .await;

    Ok(())
}

fn skill_rescan_response(
    skill_id: Uuid,
    previous_status: String,
    next_status: String,
    next_enabled: bool,
    is_safe: bool,
    findings_count: i32,
) -> Json<serde_json::Value> {
    Json(json!({
        "id": skill_id,
        "previous_review_status": previous_status,
        "review_status": next_status,
        "enabled": next_enabled,
        "is_safe": is_safe,
        "findings_count": findings_count,
    }))
}

/// POST /api/skills/{id}/rescan
pub async fn rescan_skill(
    State(state): State<AppState>,
    Path(skill_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    let skill = fetch_skill_state(&state.sqlx_pool, skill_id).await?;
    let previous_status = skill.review_status.clone();
    if previous_status != REVIEW_STATUS_SCAN_FAILED {
        return Err(Error::Validation("仅 scan_failed 状态的技能可以重扫".into()));
    }
    let content = skill
        .file_path
        .as_deref()
        .ok_or_else(|| Error::Validation("技能内容为空，无法重扫".into()))?;

    let scanner_cfg = ScannerConfig::from_env();
    if !scanner_cfg.enabled {
        return Err(scanner_disabled_error());
    }

    let scanner = SkillScanner::new(&scanner_cfg.url, scanner_cfg.timeout_ms)
        .map_err(|e| Error::Internal(format!("初始化扫描器失败: {}", e)))?;
    let (next_status, next_enabled, is_safe, findings_count) = run_skill_rescan(
        &state.sqlx_pool,
        &scanner,
        skill_id,
        &skill.name,
        content,
        &previous_status,
        skill.enabled,
    )
    .await?;

    write_skill_rescan_audit(
        &state.db_pool,
        skill_id,
        &previous_status,
        &next_status,
        is_safe,
    )
    .await?;

    Ok(skill_rescan_response(
        skill_id,
        previous_status,
        next_status,
        next_enabled,
        is_safe,
        findings_count,
    ))
}

/// POST /api/skills/{id}/yank
pub async fn yank_skill(
    State(state): State<AppState>,
    Path(skill_id): Path<Uuid>,
    Json(payload): Json<YankSkillRequest>,
) -> Result<Json<serde_json::Value>> {
    let skill = fetch_skill_state(&state.sqlx_pool, skill_id).await?;
    if skill.review_status == REVIEW_STATUS_YANKED {
        return Err(Error::Conflict("技能已处于下架状态".into()));
    }

    let note = payload.note.as_deref().map(str::trim).filter(|v| !v.is_empty());

    sqlx::query(
        "UPDATE skills
         SET review_status = $1, enabled = false, review_note = $2, updated_at = NOW()
         WHERE id = $3",
    )
    .bind(REVIEW_STATUS_YANKED)
    .bind(note)
    .bind(skill_id)
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
        "skill_yank",
        &format!(
            "技能 {} 下架: {} -> {} note={}",
            skill_id,
            skill.review_status,
            REVIEW_STATUS_YANKED,
            note.unwrap_or("-")
        ),
    )
    .await;

    Ok(Json(json!({
        "id": skill_id,
        "previous_review_status": skill.review_status,
        "review_status": REVIEW_STATUS_YANKED,
        "enabled": false,
    })))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct ScanResultRow {
    scanner_type: String,
    verdict: String,
    is_safe: bool,
    max_severity: Option<String>,
    findings_count: i32,
    findings: serde_json::Value,
    scan_duration_ms: Option<i32>,
    scanned_at: Option<chrono::DateTime<chrono::Utc>>,
    created_at: chrono::DateTime<chrono::Utc>,
}

/// GET /api/skills/{id}/scan-results
pub async fn get_skill_scan_results(
    State(state): State<AppState>,
    Path(skill_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    let row = sqlx::query_as::<_, ScanResultRow>(
        "SELECT scanner_type, verdict, is_safe, max_severity,
                findings_count, findings, scan_duration_ms,
                scanned_at, created_at
         FROM scan_results
         WHERE target_type = 'skill' AND target_id = $1
         ORDER BY scanned_at DESC NULLS LAST, created_at DESC
         LIMIT 1",
    )
    .bind(skill_id)
    .fetch_optional(&state.sqlx_pool)
    .await
    .map_err(|e| Error::Database(e.to_string()))?;

    Ok(Json(json!({
        "skill_id": skill_id,
        "scan_result": row
    })))
}

/// GET /api/plugins/{id}/scan-results
pub async fn get_plugin_scan_results(
    State(state): State<AppState>,
    Path(plugin_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    let row = sqlx::query_as::<_, ScanResultRow>(
        "SELECT scanner_type, verdict, is_safe, max_severity,
                findings_count, findings, scan_duration_ms,
                scanned_at, created_at
         FROM scan_results
         WHERE target_type = 'plugin' AND target_id = $1
         ORDER BY scanned_at DESC NULLS LAST, created_at DESC
         LIMIT 1",
    )
    .bind(plugin_id)
    .fetch_optional(&state.sqlx_pool)
    .await
    .map_err(|e| Error::Database(e.to_string()))?;

    Ok(Json(json!({
        "plugin_id": plugin_id,
        "scan_result": row
    })))
}

/// POST /api/plugins/{id}/yank
pub async fn yank_plugin(
    State(state): State<AppState>,
    Path(plugin_id): Path<Uuid>,
    Json(payload): Json<YankPluginRequest>,
) -> Result<Json<serde_json::Value>> {
    let plugin = fetch_plugin_state(&state.sqlx_pool, plugin_id).await?;
    if plugin.review_status == REVIEW_STATUS_YANKED {
        return Err(Error::Conflict("插件已处于下架状态".into()));
    }

    let note = payload.note.as_deref().map(str::trim).filter(|v| !v.is_empty());

    sqlx::query(
        "UPDATE plugins
         SET review_status = $1, enabled = false, review_note = $2, updated_at = NOW()
         WHERE id = $3",
    )
    .bind(REVIEW_STATUS_YANKED)
    .bind(note)
    .bind(plugin_id)
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
        "plugin_yank",
        &format!(
            "插件 {} 下架: {} -> {} note={}",
            plugin_id,
            plugin.review_status,
            REVIEW_STATUS_YANKED,
            note.unwrap_or("-")
        ),
    )
    .await;

    Ok(Json(json!({
        "id": plugin_id,
        "previous_review_status": plugin.review_status,
        "review_status": REVIEW_STATUS_YANKED,
        "enabled": false,
    })))
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
    headers: HeaderMap,
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
    let uploader_id = admin_user_id_from_headers(&headers);

    sqlx::query(
        "INSERT INTO plugins (id, name, description, version, author, uploaded_by, enabled,
                              source, review_status, plugin_type, is_builtin,
                              requires_sandbox, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, false, 'admin_upload', 'pending', $7, false, $8, NOW(), NOW())",
    )
    .bind(plugin_id)
    .bind(&payload.name)
    .bind(description)
    .bind(version)
    .bind(author)
    .bind(uploader_id)
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
            WHERE rc.id = $1::uuid
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

async fn department_has_whitelist(pool: &sqlx::PgPool, dept_id: Uuid) -> Result<bool> {
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM department_skill_whitelist WHERE department_id = $1",
    )
    .bind(dept_id)
    .fetch_one(pool)
    .await
    .map_err(|e| Error::Database(e.to_string()))?;

    Ok(count > 0)
}

async fn skill_in_department_whitelist(
    pool: &sqlx::PgPool,
    dept_id: Uuid,
    skill_id: Uuid,
) -> Result<bool> {
    let exists = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*)
         FROM department_skill_whitelist
         WHERE department_id = $1 AND skill_id = $2",
    )
    .bind(dept_id)
    .bind(skill_id)
    .fetch_one(pool)
    .await
    .map_err(|e| Error::Database(e.to_string()))?;

    Ok(exists > 0)
}

async fn can_download_skill_by_token(
    pool: &sqlx::PgPool,
    skill_id: Uuid,
    client_token: Option<&str>,
) -> Result<bool> {
    let Some(dept_id) = resolve_department_id(pool, client_token).await else {
        return Ok(true);
    };

    if !department_has_whitelist(pool, dept_id).await? {
        return Ok(true);
    }

    skill_in_department_whitelist(pool, dept_id, skill_id).await
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

    if !can_download_skill_by_token(&state.sqlx_pool, skill_id, params.client_token.as_deref())
        .await?
    {
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
        invoke_count: i64,
    }

    let row = sqlx::query_as::<_, Row>(
        "SELECT name, COALESCE(description,'') AS description, invoke_count
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
