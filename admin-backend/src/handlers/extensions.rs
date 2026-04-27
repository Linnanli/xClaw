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
        enabled_after_rescan, review_status_after_rescan, REVIEW_STATUS_APPROVED,
        REVIEW_STATUS_PENDING, REVIEW_STATUS_SCAN_FAILED, REVIEW_STATUS_YANKED,
    },
    extensions_validation::{extract_skill_metadata, validate_skill_package},
    routes::write_audit_log,
    scanner::{
        FindingSeverity, ScanError, ScanResult, ScanUploadOptions, ScannerConfig, SecurityFinding,
        SecurityVerdict, SkillScanner,
    },
    skill_package::extract_skill_package,
    AppState,
};

type DbPool = deadpool_postgres::Pool;
const MAX_UPLOAD_PACKAGE_BYTES: usize = 8 * 1024 * 1024;
const PROMPT_INJECTION_RULE_ID: &str = "PROMPT_INJECTION_INSTRUCTION_OVERRIDE";
const PROMPT_INJECTION_FINDING_TITLE: &str = "检测到提示词注入指令";

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
pub struct RegistryAccessQuery {
    /// client_token 用于识别用户所属部门，按白名单过滤结果
    pub client_token: Option<String>,
    /// user_id 备用身份标识
    pub user_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RegistryDownloadQuery {
    pub slug: String,
    pub client_token: Option<String>,
    pub user_id: Option<String>,
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
    let rows = if enabled {
        let sql = format!(
            "UPDATE {} SET
                enabled = true,
                review_status = CASE
                    WHEN review_status = $1 THEN $2
                    ELSE review_status
                END,
                updated_at = NOW()
             WHERE id = $3",
            table
        );

        sqlx::query(&sql)
            .bind(REVIEW_STATUS_YANKED)
            .bind(REVIEW_STATUS_APPROVED)
            .bind(item_id)
            .execute(sqlx_pool)
            .await
            .map_err(|e| Error::Database(e.to_string()))?
            .rows_affected()
    } else {
        let sql = format!(
            "UPDATE {} SET enabled = false, updated_at = NOW() WHERE id = $1",
            table
        );

        sqlx::query(&sql)
            .bind(item_id)
            .execute(sqlx_pool)
            .await
            .map_err(|e| Error::Database(e.to_string()))?
            .rows_affected()
    };

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

    let review_status_sql = format!("SELECT review_status FROM {} WHERE id = $1", table);
    let review_status: String = sqlx::query_scalar(&review_status_sql)
        .bind(item_id)
        .fetch_one(sqlx_pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let db = db_pool
        .get()
        .await
        .map_err(|e| Error::Database(e.to_string()))?;
    write_audit_log(
        &db,
        Uuid::nil(),
        &format!("{}_toggle", table),
        &format!(
            "{} {} 已{}（review_status={}）",
            table,
            item_id,
            if enabled { "启用" } else { "禁用" },
            review_status,
        ),
    )
    .await;

    Ok(Json(
        json!({ "id": item_id, "enabled": enabled, "review_status": review_status }),
    ))
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
    pub enable_llm_scan: Option<bool>,
    pub llm_model_config_id: Option<Uuid>,
    pub llm_provider: Option<String>,
    pub llm_api_key: Option<String>,
}

struct UploadedSkillPackage {
    file_name: String,
    content: String,
    manifest_json: Option<String>,
    enable_llm_scan: Option<bool>,
    llm_model_config_id: Option<Uuid>,
    llm_provider: Option<String>,
    llm_api_key: Option<String>,
}

#[derive(Debug, Serialize)]
struct ScanRuntimeDebugView {
    use_llm: bool,
    llm_provider: String,
    llm_model: Option<String>,
    llm_base_url: Option<String>,
    llm_api_version: Option<String>,
    llm_model_config_id: Option<Uuid>,
    llm_api_key_configured: bool,
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

const PROMPT_INJECTION_MARKERS: [&str; 7] = [
    "忽略上面的要求",
    "忽略以上要求",
    "ignore previous instructions",
    "ignore above instructions",
    "reveal api key",
    "api key发给我",
    "api key 给我",
];

fn suspicious_line_snippet(content: &str, marker: &str) -> String {
    for line in content.lines() {
        if line.to_lowercase().contains(marker) {
            return limit_text(line.trim(), 220);
        }
    }
    limit_text(content.trim(), 220)
}

fn prompt_injection_snippet(content: &str) -> Option<String> {
    let lowered = content.to_lowercase();
    for marker in PROMPT_INJECTION_MARKERS {
        if lowered.contains(marker) {
            return Some(suspicious_line_snippet(content, marker));
        }
    }
    None
}

fn has_prompt_injection_finding(scan_result: &ScanResult) -> bool {
    scan_result
        .findings
        .iter()
        .any(|finding| finding.rule_id.as_deref() == Some(PROMPT_INJECTION_RULE_ID))
}

fn normalize_findings_compare_text(input: &str) -> String {
    input
        .chars()
        .filter(|ch| ch.is_alphanumeric())
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

fn is_prompt_injection_like_finding(finding: &SecurityFinding) -> bool {
    let rule = finding
        .rule_id
        .as_deref()
        .map(str::to_ascii_uppercase)
        .unwrap_or_default();
    let title = finding.title.as_deref().unwrap_or("").to_ascii_lowercase();

    rule.contains("PROMPT") || title.contains("prompt") || title.contains("提示词")
}

fn has_equivalent_prompt_injection_finding(scan_result: &ScanResult, snippet: &str) -> bool {
    let target = normalize_findings_compare_text(snippet);
    if target.is_empty() {
        return false;
    }

    scan_result.findings.iter().any(|finding| {
        if !is_prompt_injection_like_finding(finding) {
            return false;
        }

        let Some(finding_snippet) = finding.snippet.as_deref() else {
            return false;
        };
        let normalized = normalize_findings_compare_text(finding_snippet);
        if normalized.is_empty() {
            return false;
        }

        normalized == target || normalized.contains(&target) || target.contains(&normalized)
    })
}

fn guarded_max_severity(existing: Option<FindingSeverity>) -> Option<FindingSeverity> {
    match existing {
        Some(FindingSeverity::Critical) => Some(FindingSeverity::Critical),
        _ => Some(FindingSeverity::High),
    }
}

fn enforce_prompt_injection_guard(scan_result: ScanResult, content: &str) -> ScanResult {
    let Some(snippet) = prompt_injection_snippet(content) else {
        return scan_result;
    };

    if has_prompt_injection_finding(&scan_result)
        || has_equivalent_prompt_injection_finding(&scan_result, &snippet)
    {
        return scan_result;
    }

    let mut findings = scan_result.findings;
    findings.push(SecurityFinding {
        rule_id: Some(PROMPT_INJECTION_RULE_ID.to_string()),
        severity: FindingSeverity::High,
        title: Some(PROMPT_INJECTION_FINDING_TITLE.to_string()),
        file: Some("SKILL.md".to_string()),
        snippet: Some(snippet),
        recommendation: Some(
            "移除提示词注入语句（如忽略系统要求、索要密钥）后重新上传".to_string(),
        ),
    });

    let findings_count = findings.len() as i32;
    ScanResult {
        scanner_type: scan_result.scanner_type,
        verdict: SecurityVerdict::Dangerous,
        is_safe: false,
        max_severity: guarded_max_severity(scan_result.max_severity),
        findings_count,
        findings,
        scan_duration_ms: scan_result.scan_duration_ms,
    }
}

const SCANNER_ERROR_SNIPPET_LIMIT: usize = 400;

fn compact_text(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn limit_text(input: &str, max_chars: usize) -> String {
    if input.chars().count() <= max_chars {
        return input.to_string();
    }
    let trimmed: String = input.chars().take(max_chars).collect();
    format!("{}...", trimmed)
}

fn scanner_error_body_summary(body: &str) -> Option<String> {
    let compact = compact_text(body);
    if compact.is_empty() {
        return None;
    }
    Some(limit_text(&compact, SCANNER_ERROR_SNIPPET_LIMIT))
}

fn scanner_error_summary(error: &ScanError) -> String {
    match error {
        ScanError::Request(message) => {
            format!(
                "扫描服务请求失败: {}",
                limit_text(&compact_text(message), 180)
            )
        }
        ScanError::HttpStatus { status, body } => {
            if let Some(summary) = scanner_error_body_summary(body) {
                return format!("扫描服务返回异常状态码 {}: {}", status, summary);
            }
            format!("扫描服务返回异常状态码 {}", status)
        }
        ScanError::Parse(message) => {
            format!(
                "扫描服务响应解析失败: {}",
                limit_text(&compact_text(message), 180)
            )
        }
    }
}

fn scanner_unavailable_error(error: &ScanError) -> Error {
    Error::Validation(format!(
        "安全扫描服务未启动或不可用: {}",
        scanner_error_summary(error)
    ))
}

fn parse_optional_bool(input: Option<&str>) -> Option<bool> {
    input.and_then(|value| match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" => Some(false),
        _ => None,
    })
}

fn build_scan_runtime_debug_view(
    payload: &SkillUploadRequest,
    scan_options: &ScanUploadOptions,
) -> ScanRuntimeDebugView {
    ScanRuntimeDebugView {
        use_llm: scan_options.use_llm,
        llm_provider: scan_options.llm_provider.clone(),
        llm_model: scan_options.llm_model.clone(),
        llm_base_url: scan_options.llm_base_url.clone(),
        llm_api_version: scan_options.llm_api_version.clone(),
        llm_model_config_id: payload.llm_model_config_id,
        llm_api_key_configured: scan_options
            .llm_api_key
            .as_deref()
            .map(str::trim)
            .is_some_and(|v| !v.is_empty()),
    }
}

fn sanitize_optional_text(input: Option<String>) -> Option<String> {
    input
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn resolve_llm_provider(
    provider_from_upload: Option<String>,
    provider_from_config: &str,
) -> Result<String> {
    let provider = sanitize_optional_text(provider_from_upload)
        .unwrap_or_else(|| provider_from_config.to_string())
        .to_ascii_lowercase();

    if provider == "anthropic" || provider == "openai" {
        return Ok(provider);
    }

    Err(Error::Validation(format!(
        "不支持的 LLM provider: {}（仅支持 anthropic/openai）",
        provider
    )))
}

fn resolve_scanner_provider_from_model(provider: &str, api_format: &str) -> String {
    let format_normalized = api_format.trim().to_ascii_lowercase();
    if format_normalized == "anthropic" || provider.eq_ignore_ascii_case("anthropic") {
        return "anthropic".to_string();
    }
    "openai".to_string()
}

/// litellm 需要 `provider/model` 格式才能路由到正确的 SDK。
/// 如果 model_id 本身不含 `/`，自动加上 scanner provider 前缀。
fn prefix_model_for_litellm(model_id: &str, scanner_provider: &str) -> String {
    if model_id.contains('/') {
        return model_id.to_string();
    }
    format!("{}/{}", scanner_provider, model_id)
}

#[derive(Debug, sqlx::FromRow)]
struct ModelConfigForScan {
    model_id: String,
    provider: String,
    api_key: Option<String>,
    enabled: bool,
    api_format: String,
    llm_base_url: Option<String>,
    llm_api_version: Option<String>,
}

async fn load_model_config_for_scan(
    sqlx_pool: &sqlx::PgPool,
    model_config_id: Uuid,
) -> Result<ModelConfigForScan> {
    let row = sqlx::query_as::<_, ModelConfigForScan>(
        "SELECT model_id, provider, api_key, enabled,
            COALESCE(extra_config->>'api_format', 'openai') AS api_format,
            NULLIF(api_base_url, '') AS llm_base_url,
            NULLIF(extra_config->>'api_version', '') AS llm_api_version
         FROM model_configs
         WHERE id = $1",
    )
    .bind(model_config_id)
    .fetch_optional(sqlx_pool)
    .await
    .map_err(|e| Error::Database(e.to_string()))?
    .ok_or_else(|| Error::Validation("所选模型配置不存在".into()))?;

    Ok(row)
}

async fn build_scan_upload_options(
    scanner_cfg: &ScannerConfig,
    sqlx_pool: &sqlx::PgPool,
    payload: &SkillUploadRequest,
) -> Result<ScanUploadOptions> {
    let mut options = ScanUploadOptions::from_config(scanner_cfg);

    if let Some(enable_llm_scan) = payload.enable_llm_scan {
        options.use_llm = enable_llm_scan;
    }

    if let Some(model_config_id) = payload.llm_model_config_id {
        let model = load_model_config_for_scan(sqlx_pool, model_config_id).await?;
        if !model.enabled {
            return Err(Error::Validation(
                "所选模型配置已禁用，请选择启用中的模型".into(),
            ));
        }

        let api_key = sanitize_optional_text(model.api_key)
            .ok_or_else(|| Error::Validation("所选模型配置未配置 API Key".into()))?;

        options.llm_provider =
            resolve_scanner_provider_from_model(&model.provider, &model.api_format);
        options.llm_api_key = Some(api_key);
        options.llm_model = Some(prefix_model_for_litellm(
            &model.model_id,
            &options.llm_provider,
        ));
        options.llm_base_url = sanitize_optional_text(model.llm_base_url);
        options.llm_api_version = sanitize_optional_text(model.llm_api_version);
        return Ok(options);
    }

    options.llm_provider =
        resolve_llm_provider(payload.llm_provider.clone(), &options.llm_provider)?;

    if let Some(llm_api_key) = sanitize_optional_text(payload.llm_api_key.clone()) {
        options.llm_api_key = Some(llm_api_key);
    }

    Ok(options)
}

async fn save_scan_result(
    sqlx_pool: &sqlx::PgPool,
    skill_id: Uuid,
    scan: &ScanResult,
) -> Result<()> {
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

async fn clear_scan_results(
    sqlx_pool: &sqlx::PgPool,
    target_type: &str,
    target_id: Uuid,
) -> Result<()> {
    sqlx::query(
        "DELETE FROM scan_results
         WHERE target_type = $1 AND target_id = $2",
    )
    .bind(target_type)
    .bind(target_id)
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

const LLM_SCAN_MIN_TIMEOUT_MS: u64 = 120_000;
const SCANNER_DEFAULT_TIMEOUT_MS: u64 = 30_000;

fn effective_scan_timeout(cfg: &ScannerConfig, options: &ScanUploadOptions) -> u64 {
    if options.use_llm && cfg.timeout_ms == SCANNER_DEFAULT_TIMEOUT_MS {
        cfg.timeout_ms.max(LLM_SCAN_MIN_TIMEOUT_MS)
    } else {
        cfg.timeout_ms
    }
}

async fn scan_skill_with_guard(
    skill_name: &str,
    content: &str,
    scanner_cfg: &ScannerConfig,
    scan_options: &ScanUploadOptions,
) -> Result<ScanResult> {
    let timeout_ms = effective_scan_timeout(scanner_cfg, scan_options);
    let scanner = SkillScanner::new(&scanner_cfg.url, timeout_ms)
        .map_err(|e| Error::Internal(format!("初始化扫描器失败: {}", e)))?;

    let scan_result = scanner
        .scan_upload(skill_name, content, scan_options)
        .await
        .map_err(|error| {
            tracing::warn!(
                skill = %skill_name,
                error = %error,
                "Security scan request failed"
            );
            scanner_unavailable_error(&error)
        })?;

    Ok(enforce_prompt_injection_guard(scan_result, content))
}

fn upload_result_message(review_status: &str, scan_row: Option<&ScanResultRow>) -> &'static str {
    if review_status == REVIEW_STATUS_SCAN_FAILED {
        if let Some(row) = scan_row {
            if row.verdict == "BLOCKED" || row.verdict == "UNKNOWN" {
                return "技能包已上传，安全扫描执行异常，请查看扫描结果";
            }
        }
        return "技能包已上传，安全扫描发现风险";
    }

    if review_status == REVIEW_STATUS_PENDING {
        if let Some(row) = scan_row {
            if row.verdict == "UNKNOWN" {
                return "技能包已上传，安全扫描未执行，等待人工审核";
            }
        }
        return "技能包已上传，安全扫描通过，等待审核";
    }

    "技能包已上传"
}

async fn fetch_latest_skill_scan_row(
    sqlx_pool: &sqlx::PgPool,
    skill_id: Uuid,
) -> Result<Option<ScanResultRow>> {
    sqlx::query_as::<_, ScanResultRow>(
        "SELECT scanner_type, verdict, is_safe, max_severity,
                findings_count, findings, scan_duration_ms,
                scanned_at, created_at
         FROM scan_results
         WHERE target_type = 'skill' AND target_id = $1
         ORDER BY scanned_at DESC NULLS LAST, created_at DESC
         LIMIT 1",
    )
    .bind(skill_id)
    .fetch_optional(sqlx_pool)
    .await
    .map_err(|e| Error::Database(e.to_string()))
}

async fn build_upload_skill_response(
    sqlx_pool: &sqlx::PgPool,
    skill_id: Uuid,
    skill_name: &str,
    scan_runtime: &ScanRuntimeDebugView,
) -> Result<(StatusCode, Json<serde_json::Value>)> {
    let final_status: String = sqlx::query_scalar("SELECT review_status FROM skills WHERE id = $1")
        .bind(skill_id)
        .fetch_one(sqlx_pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let scan_row = fetch_latest_skill_scan_row(sqlx_pool, skill_id).await?;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": skill_id,
            "name": skill_name,
            "review_status": final_status,
            "scan_result": scan_row,
            "scan_runtime": scan_runtime,
            "message": upload_result_message(&final_status, scan_row.as_ref())
        })),
    ))
}

pub async fn upload_skill(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(mut payload): Json<SkillUploadRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>)> {
    validate_skill_package(&payload.content)?;
    let metadata = extract_skill_metadata(&payload.content, None)?;

    if payload.version.is_none() {
        payload.version = metadata.version;
    }
    if payload.description.is_none() {
        payload.description = metadata.description;
    }
    if payload.author.is_none() {
        payload.author = metadata.author;
    }

    upload_skill_from_payload(&state, &headers, payload, "json").await
}

pub async fn upload_skill_package(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<serde_json::Value>)> {
    let uploaded = read_skill_package_from_multipart(&mut multipart).await?;
    validate_skill_package(&uploaded.content)?;

    let metadata = extract_skill_metadata(&uploaded.content, uploaded.manifest_json.as_deref())?;
    let payload = SkillUploadRequest {
        name: metadata.name,
        content: uploaded.content,
        version: metadata.version,
        description: metadata.description,
        author: metadata.author,
        enable_llm_scan: uploaded.enable_llm_scan,
        llm_model_config_id: uploaded.llm_model_config_id,
        llm_provider: uploaded.llm_provider,
        llm_api_key: uploaded.llm_api_key,
    };

    upload_skill_from_payload(&state, &headers, payload, &uploaded.file_name).await
}

async fn read_skill_package_from_multipart(
    multipart: &mut Multipart,
) -> Result<UploadedSkillPackage> {
    let mut uploaded: Option<UploadedSkillPackage> = None;
    let mut enable_llm_scan: Option<bool> = None;
    let mut llm_model_config_id: Option<Uuid> = None;
    let mut llm_provider: Option<String> = None;
    let mut llm_api_key: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| Error::Validation(format!("读取上传字段失败: {}", e)))?
    {
        let field_name = field.name().map(ToString::to_string);

        if let Some(file_name) = field.file_name().map(ToString::to_string) {
            let bytes = read_field_bytes_limited(field, MAX_UPLOAD_PACKAGE_BYTES).await?;
            let extracted = extract_skill_package(&file_name, &bytes)?;
            uploaded = Some(UploadedSkillPackage {
                file_name,
                content: extracted.skill_content,
                manifest_json: extracted.manifest_json,
                enable_llm_scan: None,
                llm_model_config_id: None,
                llm_provider: None,
                llm_api_key: None,
            });
            continue;
        }

        let Some(name) = field_name else {
            continue;
        };

        let value = field
            .text()
            .await
            .map_err(|e| Error::Validation(format!("读取字段 {} 失败: {}", name, e)))?;

        apply_llm_field_from_multipart(
            &name,
            value,
            &mut enable_llm_scan,
            &mut llm_model_config_id,
            &mut llm_provider,
            &mut llm_api_key,
        )?;
    }

    let mut result = uploaded.ok_or_else(|| Error::Validation("未检测到上传文件字段".into()))?;
    result.enable_llm_scan = enable_llm_scan;
    result.llm_model_config_id = llm_model_config_id;
    result.llm_provider = llm_provider;
    result.llm_api_key = llm_api_key;
    Ok(result)
}

fn parse_model_config_id_from_form(value: &str) -> Result<Option<Uuid>> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let parsed = Uuid::parse_str(trimmed).map_err(|_| {
        Error::Validation(format!(
            "字段 llm_model_config_id 值无效: {}（应为 UUID）",
            value
        ))
    })?;
    Ok(Some(parsed))
}

fn parse_enable_llm_scan_from_form(value: &str) -> Result<Option<bool>> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let parsed = parse_optional_bool(Some(trimmed)).ok_or_else(|| {
        Error::Validation(format!(
            "字段 enable_llm_scan 值无效: {}（仅支持 true/false）",
            value
        ))
    })?;
    Ok(Some(parsed))
}

fn apply_llm_field_from_multipart(
    name: &str,
    value: String,
    enable_llm_scan: &mut Option<bool>,
    llm_model_config_id: &mut Option<Uuid>,
    llm_provider: &mut Option<String>,
    llm_api_key: &mut Option<String>,
) -> Result<()> {
    match name {
        "enable_llm_scan" => {
            if let Some(parsed) = parse_enable_llm_scan_from_form(&value)? {
                *enable_llm_scan = Some(parsed);
            }
        }
        "llm_model_config_id" => {
            if let Some(parsed) = parse_model_config_id_from_form(&value)? {
                *llm_model_config_id = Some(parsed);
            }
        }
        "llm_provider" => {
            *llm_provider = sanitize_optional_text(Some(value));
        }
        "llm_api_key" => {
            *llm_api_key = sanitize_optional_text(Some(value));
        }
        _ => {}
    }

    Ok(())
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

async fn ensure_scanner_service_ready(scanner_cfg: &ScannerConfig) -> Result<()> {
    let scanner = SkillScanner::new(&scanner_cfg.url, scanner_cfg.timeout_ms)
        .map_err(|e| Error::Validation(format!("安全扫描服务未启动或不可用: {}", e)))?;

    scanner
        .check_health()
        .await
        .map_err(|e| Error::Validation(format!("安全扫描服务未启动或不可用: {}", e)))?;

    Ok(())
}

async fn upsert_uploaded_skill(
    sqlx_pool: &sqlx::PgPool,
    skill_name: &str,
    version: &str,
    description: &str,
    author: &str,
    uploader_id: Option<Uuid>,
    content: &str,
) -> Result<Uuid> {
    sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO skills (
            id, name, description, version, author, uploaded_by, file_path,
            enabled, source, review_status, is_builtin, created_at, updated_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, false, 'admin_upload', 'scanning', false, NOW(), NOW())
        ON CONFLICT (name) DO UPDATE
        SET
            description = EXCLUDED.description,
            version = EXCLUDED.version,
            author = EXCLUDED.author,
            uploaded_by = EXCLUDED.uploaded_by,
            file_path = EXCLUDED.file_path,
            enabled = false,
            source = 'admin_upload',
            review_status = 'scanning',
            review_note = NULL,
            reviewed_by = NULL,
            reviewed_at = NULL,
            updated_at = NOW()
        RETURNING id",
    )
    .bind(Uuid::new_v4())
    .bind(skill_name)
    .bind(description)
    .bind(version)
    .bind(author)
    .bind(uploader_id)
    .bind(content)
    .fetch_one(sqlx_pool)
    .await
    .map_err(|e| Error::Database(e.to_string()))
}

async fn finalize_upload_review_status(
    sqlx_pool: &sqlx::PgPool,
    skill_id: Uuid,
    scan_result: Option<&ScanResult>,
) -> Result<()> {
    if let Some(scan_result) = scan_result {
        save_scan_result(sqlx_pool, skill_id, scan_result).await?;
        let next_status = if scan_result.is_safe {
            REVIEW_STATUS_PENDING
        } else {
            REVIEW_STATUS_SCAN_FAILED
        };
        return update_skill_review_status(sqlx_pool, skill_id, next_status)
            .await
            .map(|_| ());
    }

    // SCANNER_ENABLED=false 时写入占位扫描结果，避免审核门禁因无记录而卡死。
    save_scan_result(sqlx_pool, skill_id, &scanner_disabled_placeholder_result()).await?;

    update_skill_review_status(sqlx_pool, skill_id, REVIEW_STATUS_PENDING)
        .await
        .map(|_| ())
}

fn scanner_disabled_placeholder_result() -> ScanResult {
    ScanResult {
        scanner_type: "scanner-disabled".to_string(),
        verdict: SecurityVerdict::Unknown,
        is_safe: false,
        max_severity: Some(FindingSeverity::Unknown),
        findings_count: 1,
        findings: vec![SecurityFinding {
            rule_id: Some("SCANNER_DISABLED".to_string()),
            severity: FindingSeverity::Unknown,
            title: Some("安全扫描未执行".to_string()),
            file: Some("SKILL.md".to_string()),
            snippet: None,
            recommendation: Some("启用扫描服务并执行重扫后再审核".to_string()),
        }],
        scan_duration_ms: None,
    }
}

async fn upload_skill_from_payload(
    state: &AppState,
    headers: &HeaderMap,
    payload: SkillUploadRequest,
    source_label: &str,
) -> Result<(StatusCode, Json<serde_json::Value>)> {
    let uploader_id = admin_user_id_from_headers(headers);
    let skill_name = payload.name.clone();
    let version = payload
        .version
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("1.0.0");
    let description = payload.description.as_deref().unwrap_or("");
    let author = payload.author.as_deref().unwrap_or("");
    let scanner_cfg = ScannerConfig::from_env();
    let scan_options = build_scan_upload_options(&scanner_cfg, &state.sqlx_pool, &payload).await?;
    let scan_runtime = build_scan_runtime_debug_view(&payload, &scan_options);

    tracing::info!(
        skill = %skill_name,
        use_llm = scan_runtime.use_llm,
        llm_provider = %scan_runtime.llm_provider,
        llm_model = ?scan_runtime.llm_model,
        llm_base_url = ?scan_runtime.llm_base_url,
        llm_api_version = ?scan_runtime.llm_api_version,
        llm_model_config_id = ?scan_runtime.llm_model_config_id,
        llm_api_key_configured = scan_runtime.llm_api_key_configured,
        "Resolved scan runtime options for skill upload"
    );

    if scanner_cfg.enabled {
        ensure_scanner_service_ready(&scanner_cfg).await?;
    }

    let guarded_scan_result = if scanner_cfg.enabled {
        Some(
            scan_skill_with_guard(&skill_name, &payload.content, &scanner_cfg, &scan_options)
                .await?,
        )
    } else {
        None
    };

    let persisted_skill_id = upsert_uploaded_skill(
        &state.sqlx_pool,
        &skill_name,
        version,
        description,
        author,
        uploader_id,
        &payload.content,
    )
    .await?;

    // Re-upload should replace the previous review application context and stale scan output.
    clear_scan_results(&state.sqlx_pool, "skill", persisted_skill_id).await?;

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
            skill_name, persisted_skill_id, source_label
        ),
    )
    .await;

    finalize_upload_review_status(
        &state.sqlx_pool,
        persisted_skill_id,
        guarded_scan_result.as_ref(),
    )
    .await?;

    build_upload_skill_response(
        &state.sqlx_pool,
        persisted_skill_id,
        &skill_name,
        &scan_runtime,
    )
    .await
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

    ensure_scan_result_exists_for_review(sqlx_pool, table, item_id).await?;

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

fn scan_target_type_for_table(table: &str) -> Option<&'static str> {
    match table {
        "skills" => Some("skill"),
        "plugins" => Some("plugin"),
        _ => None,
    }
}

async fn ensure_scan_result_exists_for_review(
    sqlx_pool: &sqlx::PgPool,
    table: &str,
    item_id: Uuid,
) -> Result<()> {
    let Some(target_type) = scan_target_type_for_table(table) else {
        return Ok(());
    };

    let latest_scan_exists = sqlx::query_scalar::<_, i32>(
        "SELECT 1
         FROM scan_results
         WHERE target_type = $1 AND target_id = $2
         ORDER BY scanned_at DESC NULLS LAST, created_at DESC
         LIMIT 1",
    )
    .bind(target_type)
    .bind(item_id)
    .fetch_optional(sqlx_pool)
    .await
    .map_err(|e| Error::Database(e.to_string()))?;

    if latest_scan_exists.is_none() {
        return Err(Error::Validation("未检测到扫描结果，禁止审核".into()));
    }

    Ok(())
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
    scan_options: &ScanUploadOptions,
    skill_id: Uuid,
    skill_name: &str,
    content: &str,
    current_status: &str,
    current_enabled: bool,
) -> Result<(String, bool, bool, i32)> {
    match scanner.scan_upload(skill_name, content, scan_options).await {
        Ok(scan_result) => {
            let guarded_result = enforce_prompt_injection_guard(scan_result, content);
            let (next_status, next_enabled) = persist_rescan_result(
                sqlx_pool,
                skill_id,
                current_status,
                current_enabled,
                &guarded_result,
            )
            .await?;

            Ok((
                next_status,
                next_enabled,
                guarded_result.is_safe,
                guarded_result.findings_count,
            ))
        }
        Err(error) => {
            tracing::warn!(
                skill_id = %skill_id,
                skill = %skill_name,
                error = %error,
                "Skill rescan request failed"
            );
            Err(scanner_unavailable_error(&error))
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

fn can_rescan_skill(review_status: &str) -> bool {
    review_status == REVIEW_STATUS_SCAN_FAILED || review_status == REVIEW_STATUS_PENDING
}

fn scanner_disabled_error() -> Error {
    Error::Validation("安全扫描器未启用，无法执行重扫".into())
}

/// POST /api/skills/{id}/rescan
pub async fn rescan_skill(
    State(state): State<AppState>,
    Path(skill_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    let skill = fetch_skill_state(&state.sqlx_pool, skill_id).await?;
    let previous_status = skill.review_status.clone();
    if !can_rescan_skill(&previous_status) {
        return Err(Error::Validation(
            "仅 pending 或 scan_failed 状态的技能可以重扫".into(),
        ));
    }
    let content = skill
        .file_path
        .as_deref()
        .ok_or_else(|| Error::Validation("技能内容为空，无法重扫".into()))?;

    let scanner_cfg = ScannerConfig::from_env();
    if !scanner_cfg.enabled {
        return Err(scanner_disabled_error());
    }

    ensure_scanner_service_ready(&scanner_cfg).await?;

    let scan_options = ScanUploadOptions::from_config(&scanner_cfg);
    let timeout_ms = effective_scan_timeout(&scanner_cfg, &scan_options);

    let scanner = SkillScanner::new(&scanner_cfg.url, timeout_ms)
        .map_err(|e| Error::Internal(format!("初始化扫描器失败: {}", e)))?;

    let (next_status, next_enabled, is_safe, findings_count) = run_skill_rescan(
        &state.sqlx_pool,
        &scanner,
        &scan_options,
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

    let note = payload
        .note
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());

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

    let note = payload
        .note
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());

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
// 未提供 token 或 token 无效时，search 返回空结果，download/detail 拒绝访问。

/// 从 client_token 解析出部门 ID（查 registered_clients 表）
async fn resolve_department_id(pool: &sqlx::PgPool, client_token: Option<&str>) -> Option<Uuid> {
    let token = client_token?;
    let client_id = Uuid::parse_str(token).ok()?;

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
    .bind(client_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .and_then(|r| r.department_id)
}

fn registry_access_token<'a>(
    client_token: Option<&'a str>,
    user_id: Option<&'a str>,
) -> Option<&'a str> {
    client_token.or(user_id)
}

async fn resolve_effective_whitelist_department(
    pool: &sqlx::PgPool,
    start_department_id: Uuid,
) -> Result<Option<Uuid>> {
    let mut current_id = Some(start_department_id);

    for _ in 0..10 {
        let Some(dept_id) = current_id else {
            break;
        };

        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM department_skill_whitelist WHERE department_id = $1",
        )
        .bind(dept_id)
        .fetch_one(pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        if count > 0 {
            return Ok(Some(dept_id));
        }

        current_id = sqlx::query_scalar::<_, Option<Uuid>>(
            "SELECT parent_id FROM departments WHERE id = $1",
        )
        .bind(dept_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .flatten();
    }

    Ok(None)
}

/// GET /api/v1/search?q=xxx&client_token=yyy
pub async fn registry_search(
    State(state): State<AppState>,
    Query(params): Query<RegistrySearchQuery>,
) -> Result<Json<serde_json::Value>> {
    let query = params.q.as_deref().unwrap_or("").to_lowercase();
    let token = registry_access_token(params.client_token.as_deref(), params.user_id.as_deref());
    let Some(dept_id) = resolve_department_id(&state.sqlx_pool, token).await else {
        tracing::warn!(
            has_identity = token.is_some(),
            "registry_search: missing or invalid client token"
        );
        return Ok(Json(json!({ "results": [] })));
    };

    let skills = fetch_skills_for_department(&state.sqlx_pool, dept_id, &query).await?;

    Ok(Json(json!({ "results": skills })))
}

/// 按部门查询：有白名单则过滤，无白名单则返回全部
async fn fetch_skills_for_department(
    pool: &sqlx::PgPool,
    dept_id: Uuid,
    query: &str,
) -> Result<Vec<serde_json::Value>> {
    if let Some(effective_dept_id) = resolve_effective_whitelist_department(pool, dept_id).await? {
        fetch_skills_by_whitelist(pool, effective_dept_id, query).await
    } else {
        fetch_all_enabled_skills(pool, query).await
    }
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
        return Ok(false);
    };

    let Some(effective_dept_id) = resolve_effective_whitelist_department(pool, dept_id).await?
    else {
        return Ok(true);
    };

    skill_in_department_whitelist(pool, effective_dept_id, skill_id).await
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
        "displayName": s.name,
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

    let token = registry_access_token(params.client_token.as_deref(), params.user_id.as_deref());
    if !can_download_skill_by_token(&state.sqlx_pool, skill_id, token).await? {
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
    Query(params): Query<RegistryAccessQuery>,
) -> Result<Json<serde_json::Value>> {
    let skill_id = Uuid::parse_str(&slug).map_err(|_| Error::Validation("slug 格式无效".into()))?;

    let token = registry_access_token(params.client_token.as_deref(), params.user_id.as_deref());
    if !can_download_skill_by_token(&state.sqlx_pool, skill_id, token).await? {
        return Err(Error::NotFound("技能不可用".into()));
    }

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
            "displayName": row.name,
            "summary": row.description,
            "updatedAt": null,
            "stats": {
                "installsCurrent": row.invoke_count,
                "downloads": row.invoke_count,
                "stars": null,
            },
        },
        "owner": null,
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
