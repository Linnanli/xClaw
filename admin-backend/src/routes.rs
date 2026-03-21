use crate::auth::AuthManager;
use crate::error::{Error, Result};
use crate::handlers::{
    get_dlp_policies_handler, get_policies_handler, get_policy_version_handler,
    get_sensitive_ops_policies_handler,
};
use crate::models::{CreateUserRequest, LoginRequest, LoginResponse, RefreshTokenRequest, CreateRoleRequest, UpdateRoleRequest, AssignPermissionsRequest, CreateDlpRuleRequest, UpdateDlpRuleRequest, CreateDictionaryRequest, UpdateDictionaryRequest};
use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde_json::json;
use serde::Deserialize;
use uuid::Uuid;

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/api/auth/register", post(register))
        .route("/api/auth/login", post(login))
        .route("/api/auth/refresh", post(refresh_token))
        .route("/api/users", get(get_users).post(create_user))
        .route("/api/users/{id}", get(get_user).delete(delete_user))
        .route("/api/users/{id}/roles", get(get_user_roles).post(assign_user_roles))
        .route("/api/roles", get(get_roles).post(create_role))
        .route("/api/roles/{id}", get(get_role).put(update_role).delete(delete_role))
        .route("/api/roles/{id}/permissions", post(assign_permissions))
        .route("/api/permissions", get(get_permissions))
        .route("/api/dlp-rules", get(get_dlp_rules).post(create_dlp_rule))
        .route("/api/dlp-rules/batch/status", post(batch_update_dlp_status))
        .route("/api/dlp-rules/batch/delete", post(batch_delete_dlp_rules))
        .route("/api/dlp-rules/export", get(export_dlp_rules))
        .route("/api/dlp-rules/import", post(import_dlp_rules))
        .route("/api/dlp-rules/{id}", put(update_dlp_rule).delete(delete_dlp_rule))
        .route("/api/dlp-dictionaries", get(get_dictionaries).post(create_dictionary))
        .route("/api/dlp-dictionaries/{id}", get(get_dictionary).put(update_dictionary).delete(delete_dictionary))
        .route("/api/audit-logs", get(get_audit_logs))
        .route("/api/audit-logs/report", post(report_audit_event))
        .route("/api/sensitive-operations", get(get_sensitive_operations))
        // 新增：策略查询 API（供 Desktop Client 使用）
        .route("/api/policies", get(get_policies_handler))
        .route("/api/policies/dlp", get(get_dlp_policies_handler))
        .route("/api/policies/sensitive-ops", get(get_sensitive_ops_policies_handler))
        .route("/api/policies/version", get(get_policy_version_handler))
        .with_state(state)
}

async fn health_check() -> impl IntoResponse {
    Json(json!({ "status": "ok" }))
}

/// 写入审计日志的辅助函数（不阻塞主流程，失败仅打印日志）
async fn write_audit_log(
    client: &deadpool_postgres::Object,
    user_id: Uuid,
    action: &str,
    details: &str,
) {
    let id = Uuid::new_v4();
    let now = chrono::Utc::now();
    if let Err(e) = client
        .execute(
            "INSERT INTO audit_logs (id, user_id, action, details, created_at) VALUES ($1, $2, $3, $4, $5)",
            &[&id, &user_id, &action, &details, &now],
        )
        .await
    {
        eprintln!("Failed to write audit log: {}", e);
    }
}

async fn register(
    State(state): State<AppState>,
    Json(payload): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>)> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    // Check if user already exists
    let existing = client
        .query_opt("SELECT id FROM users WHERE username = $1", &[&payload.username])
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    if existing.is_some() {
        return Err(Error::UserExists);
    }

    let auth = AuthManager::new(std::env::var("JWT_SECRET").unwrap_or_else(|_| "secret".to_string()));
    let password_hash = auth.hash_password(&payload.password)?;

    let user_id = Uuid::new_v4();
    let now = chrono::Utc::now();

    client
        .execute(
            "INSERT INTO users (id, username, email, password_hash, created_at, updated_at) 
             VALUES ($1, $2, $3, $4, $5, $6)",
            &[&user_id, &payload.username, &payload.email, &password_hash, &now, &now],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": user_id,
            "username": payload.username,
            "email": payload.email,
            "created_at": now
        })),
    ))
}

async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<LoginResponse>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let row = client
        .query_opt(
            "SELECT id, password_hash FROM users WHERE username = $1",
            &[&payload.username],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .ok_or(Error::InvalidCredentials)?;

    let user_id: Uuid = row.get(0);
    let password_hash: String = row.get(1);

    let auth = AuthManager::new(std::env::var("JWT_SECRET").unwrap_or_else(|_| "secret".to_string()));
    auth.verify_password(&payload.password, &password_hash)?;

    let access_token = auth.generate_access_token(&user_id.to_string())?;
    let refresh_token = auth.generate_refresh_token(&user_id.to_string())?;

    // 记录登录审计日志
    write_audit_log(&client, user_id, "login", &format!("用户 {} 登录成功", payload.username)).await;

    Ok(Json(LoginResponse {
        access_token,
        refresh_token,
        expires_in: 3600,
    }))
}

async fn refresh_token(
    State(_state): State<AppState>,
    Json(payload): Json<RefreshTokenRequest>,
) -> Result<Json<LoginResponse>> {
    let auth = AuthManager::new(std::env::var("JWT_SECRET").unwrap_or_else(|_| "secret".to_string()));
    let claims = auth.verify_token(&payload.refresh_token)?;

    let access_token = auth.generate_access_token(&claims.sub)?;
    let new_refresh_token = auth.generate_refresh_token(&claims.sub)?;

    Ok(Json(LoginResponse {
        access_token,
        refresh_token: new_refresh_token,
        expires_in: 3600,
    }))
}

async fn get_users(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let rows = client
        .query(
            "SELECT u.id, u.username, u.email, u.created_at, u.updated_at
             FROM users u
             ORDER BY u.created_at DESC",
            &[],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let mut users = Vec::new();
    for row in rows {
        let user_id: Uuid = row.get(0);
        
        // Get user roles
        let role_rows = client
            .query(
                "SELECT r.id, r.name
                 FROM roles r
                 INNER JOIN user_roles ur ON r.id = ur.role_id
                 WHERE ur.user_id = $1",
                &[&user_id],
            )
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        let roles: Vec<_> = role_rows
            .iter()
            .map(|r| {
                json!({
                    "id": r.get::<_, Uuid>(0),
                    "name": r.get::<_, String>(1),
                })
            })
            .collect();

        users.push(json!({
            "id": user_id,
            "username": row.get::<_, String>(1),
            "email": row.get::<_, String>(2),
            "created_at": row.get::<_, chrono::DateTime<chrono::Utc>>(3),
            "updated_at": row.get::<_, chrono::DateTime<chrono::Utc>>(4),
            "roles": roles,
        }));
    }

    Ok(Json(json!({ "users": users })))
}

async fn delete_user(
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
) -> Result<StatusCode> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let result = client
        .execute("DELETE FROM users WHERE id = $1", &[&user_id])
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    if result == 0 {
        return Err(Error::UserNotFound);
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn get_user(
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let row = client
        .query_opt(
            "SELECT id, username, email, created_at, updated_at FROM users WHERE id = $1",
            &[&user_id],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .ok_or(Error::UserNotFound)?;

    Ok(Json(json!({
        "id": row.get::<_, Uuid>(0),
        "username": row.get::<_, String>(1),
        "email": row.get::<_, String>(2),
        "created_at": row.get::<_, chrono::DateTime<chrono::Utc>>(3),
        "updated_at": row.get::<_, chrono::DateTime<chrono::Utc>>(4),
    })))
}

/// 审计日志查询参数
#[derive(Debug, Deserialize)]
struct AuditLogQuery {
    page: Option<i64>,
    page_size: Option<i64>,
}

async fn get_audit_logs(
    State(state): State<AppState>,
    Query(params): Query<AuditLogQuery>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let page = params.page.unwrap_or(1).max(1);
    let page_size = params.page_size.unwrap_or(50).clamp(1, 200);
    let offset = (page - 1) * page_size;

    // 查询总数
    let count_row = client
        .query_one("SELECT COUNT(*) FROM audit_logs", &[])
        .await
        .map_err(|e| Error::Database(e.to_string()))?;
    let total: i64 = count_row.get(0);

    // LEFT JOIN users 获取用户名
    let rows = client
        .query(
            "SELECT a.id, a.user_id, u.username, a.action, a.details, a.created_at
             FROM audit_logs a
             LEFT JOIN users u ON a.user_id = u.id
             ORDER BY a.created_at DESC
             LIMIT $1 OFFSET $2",
            &[&page_size, &offset],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let logs: Vec<_> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<_, Uuid>(0),
                "user_id": r.get::<_, Option<Uuid>>(1),
                "username": r.get::<_, Option<String>>(2),
                "action": r.get::<_, String>(3),
                "details": r.get::<_, String>(4),
                "created_at": r.get::<_, chrono::DateTime<chrono::Utc>>(5),
            })
        })
        .collect();

    Ok(Json(json!({
        "logs": logs,
        "total": total,
        "page": page,
        "page_size": page_size,
        "total_pages": (total as f64 / page_size as f64).ceil() as i64,
    })))
}

async fn get_dlp_rules(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let rows = client
        .query(
            "SELECT id, name, pattern, replacement, severity, description, enabled, category, created_at, updated_at, rule_type, rule_config 
             FROM dlp_rules 
             ORDER BY created_at DESC",
            &[],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let rules: Vec<_> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<_, Uuid>(0),
                "name": r.get::<_, String>(1),
                "pattern": r.get::<_, String>(2),
                "replacement": r.get::<_, Option<String>>(3),
                "severity": r.get::<_, String>(4),
                "description": r.get::<_, Option<String>>(5),
                "enabled": r.get::<_, bool>(6),
                "category": r.get::<_, String>(7),
                "created_at": r.get::<_, chrono::DateTime<chrono::Utc>>(8),
                "updated_at": r.get::<_, chrono::DateTime<chrono::Utc>>(9),
                "rule_type": r.get::<_, String>(10),
                "rule_config": r.get::<_, Option<serde_json::Value>>(11),
            })
        })
        .collect();

    Ok(Json(json!({ "rules": rules })))
}

async fn create_dlp_rule(
    State(state): State<AppState>,
    Json(payload): Json<CreateDlpRuleRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>)> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let rule_id = Uuid::new_v4();
    let now = chrono::Utc::now();

    client
        .execute(
            "INSERT INTO dlp_rules (id, name, pattern, replacement, severity, description, enabled, category, rule_type, rule_config, created_at, updated_at) 
             VALUES ($1, $2, $3, $4, $5, $6, true, $7, $8, $9, $10, $11)",
            &[&rule_id, &payload.name, &payload.pattern, &payload.replacement, &payload.severity, &payload.description, &payload.category, &payload.rule_type, &payload.rule_config, &now, &now],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    // 记录审计日志
    let system_user = Uuid::nil();
    write_audit_log(&client, system_user, "create_dlp_rule", &format!("创建 DLP 规则: {} (类型: {})", payload.name, payload.rule_type)).await;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": rule_id,
            "name": payload.name,
            "pattern": payload.pattern,
            "replacement": payload.replacement,
            "severity": payload.severity,
            "description": payload.description,
            "enabled": true,
            "category": payload.category,
            "rule_type": payload.rule_type,
            "rule_config": payload.rule_config,
            "created_at": now,
            "updated_at": now,
        })),
    ))
}

async fn update_dlp_rule(
    State(state): State<AppState>,
    Path(rule_id): Path<Uuid>,
    Json(payload): Json<UpdateDlpRuleRequest>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    // Check if rule exists
    let existing = client
        .query_opt("SELECT id FROM dlp_rules WHERE id = $1", &[&rule_id])
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    if existing.is_none() {
        return Err(Error::NotFound("DLP rule not found".to_string()));
    }

    let now = chrono::Utc::now();
    let mut updates = vec!["updated_at = $1".to_string()];
    let mut params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = vec![&now];
    let mut param_count = 2;

    if let Some(ref name) = payload.name {
        updates.push(format!("name = ${}", param_count));
        params.push(name);
        param_count += 1;
    }

    if let Some(ref pattern) = payload.pattern {
        updates.push(format!("pattern = ${}", param_count));
        params.push(pattern);
        param_count += 1;
    }

    if let Some(ref replacement) = payload.replacement {
        updates.push(format!("replacement = ${}", param_count));
        params.push(replacement);
        param_count += 1;
    }

    if let Some(ref severity) = payload.severity {
        updates.push(format!("severity = ${}", param_count));
        params.push(severity);
        param_count += 1;
    }

    if let Some(ref description) = payload.description {
        updates.push(format!("description = ${}", param_count));
        params.push(description);
        param_count += 1;
    }

    if let Some(ref enabled) = payload.enabled {
        updates.push(format!("enabled = ${}", param_count));
        params.push(enabled);
        param_count += 1;
    }

    if let Some(ref category) = payload.category {
        updates.push(format!("category = ${}", param_count));
        params.push(category);
        param_count += 1;
    }

    if let Some(ref rule_type) = payload.rule_type {
        updates.push(format!("rule_type = ${}", param_count));
        params.push(rule_type);
        param_count += 1;
    }

    if let Some(ref rule_config) = payload.rule_config {
        updates.push(format!("rule_config = ${}", param_count));
        params.push(rule_config);
        param_count += 1;
    }

    params.push(&rule_id);

    let query = format!(
        "UPDATE dlp_rules SET {} WHERE id = ${} RETURNING id, name, pattern, replacement, severity, description, enabled, category, created_at, updated_at, rule_type, rule_config",
        updates.join(", "),
        param_count
    );

    let row = client
        .query_one(&query, &params)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    // 记录审计日志
    let system_user = Uuid::nil();
    write_audit_log(&client, system_user, "update_dlp_rule", &format!("更新 DLP 规则: {} ({})", row.get::<_, String>(1), rule_id)).await;

    Ok(Json(json!({
        "id": row.get::<_, Uuid>(0),
        "name": row.get::<_, String>(1),
        "pattern": row.get::<_, String>(2),
        "replacement": row.get::<_, Option<String>>(3),
        "severity": row.get::<_, String>(4),
        "description": row.get::<_, Option<String>>(5),
        "enabled": row.get::<_, bool>(6),
        "category": row.get::<_, String>(7),
        "created_at": row.get::<_, chrono::DateTime<chrono::Utc>>(8),
        "updated_at": row.get::<_, chrono::DateTime<chrono::Utc>>(9),
        "rule_type": row.get::<_, String>(10),
        "rule_config": row.get::<_, Option<serde_json::Value>>(11),
    })))
}

async fn delete_dlp_rule(
    State(state): State<AppState>,
    Path(rule_id): Path<Uuid>,
) -> Result<StatusCode> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let result = client
        .execute("DELETE FROM dlp_rules WHERE id = $1", &[&rule_id])
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    if result == 0 {
        return Err(Error::NotFound("DLP rule not found".to_string()));
    }

    // 记录审计日志
    let system_user = Uuid::nil();
    write_audit_log(&client, system_user, "delete_dlp_rule", &format!("删除 DLP 规则: {}", rule_id)).await;

    Ok(StatusCode::NO_CONTENT)
}

// --- 批量操作和导入导出 ---

#[derive(Debug, Deserialize)]
struct BatchStatusRequest {
    ids: Vec<Uuid>,
    enabled: bool,
}

#[derive(Debug, Deserialize)]
struct BatchDeleteRequest {
    ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize)]
struct ImportDlpRulesRequest {
    rules: Vec<CreateDlpRuleRequest>,
}

async fn batch_update_dlp_status(
    State(state): State<AppState>,
    Json(payload): Json<BatchStatusRequest>,
) -> Result<Json<serde_json::Value>> {
    if payload.ids.is_empty() {
        return Err(Error::Validation("ids 不能为空".to_string()));
    }

    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let now = chrono::Utc::now();
    let mut updated = 0u64;

    for id in &payload.ids {
        let result = client
            .execute(
                "UPDATE dlp_rules SET enabled = $1, updated_at = $2 WHERE id = $3",
                &[&payload.enabled, &now, id],
            )
            .await
            .map_err(|e| Error::Database(e.to_string()))?;
        updated += result;
    }

    // 记录审计日志
    let system_user = Uuid::nil();
    let action_text = if payload.enabled { "批量启用" } else { "批量禁用" };
    write_audit_log(&client, system_user, "batch_update_dlp_status", &format!("{} {} 条 DLP 规则", action_text, updated)).await;

    Ok(Json(json!({
        "updated": updated,
        "message": format!("成功更新 {} 条规则", updated),
    })))
}

async fn batch_delete_dlp_rules(
    State(state): State<AppState>,
    Json(payload): Json<BatchDeleteRequest>,
) -> Result<Json<serde_json::Value>> {
    if payload.ids.is_empty() {
        return Err(Error::Validation("ids 不能为空".to_string()));
    }

    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let mut deleted = 0u64;

    for id in &payload.ids {
        let result = client
            .execute("DELETE FROM dlp_rules WHERE id = $1", &[id])
            .await
            .map_err(|e| Error::Database(e.to_string()))?;
        deleted += result;
    }

    // 记录审计日志
    let system_user = Uuid::nil();
    write_audit_log(&client, system_user, "batch_delete_dlp_rules", &format!("批量删除 {} 条 DLP 规则", deleted)).await;

    Ok(Json(json!({
        "deleted": deleted,
        "message": format!("成功删除 {} 条规则", deleted),
    })))
}

async fn export_dlp_rules(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let rows = client
        .query(
            "SELECT name, pattern, replacement, severity, description, enabled, category, rule_type, rule_config FROM dlp_rules ORDER BY created_at DESC",
            &[],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let rules: Vec<_> = rows
        .iter()
        .map(|r| {
            json!({
                "name": r.get::<_, String>(0),
                "pattern": r.get::<_, String>(1),
                "replacement": r.get::<_, Option<String>>(2),
                "severity": r.get::<_, String>(3),
                "description": r.get::<_, Option<String>>(4),
                "enabled": r.get::<_, bool>(5),
                "category": r.get::<_, String>(6),
                "rule_type": r.get::<_, String>(7),
                "rule_config": r.get::<_, Option<serde_json::Value>>(8),
            })
        })
        .collect();

    Ok(Json(json!({
        "version": "1.0",
        "exported_at": chrono::Utc::now(),
        "count": rules.len(),
        "rules": rules,
    })))
}

async fn import_dlp_rules(
    State(state): State<AppState>,
    Json(payload): Json<ImportDlpRulesRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>)> {
    if payload.rules.is_empty() {
        return Err(Error::Validation("rules 不能为空".to_string()));
    }

    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let now = chrono::Utc::now();
    let mut imported = 0u64;
    let mut errors: Vec<String> = Vec::new();

    for rule in &payload.rules {
        let rule_id = Uuid::new_v4();
        match client
            .execute(
                "INSERT INTO dlp_rules (id, name, pattern, replacement, severity, description, enabled, category, rule_type, rule_config, created_at, updated_at) 
                 VALUES ($1, $2, $3, $4, $5, $6, true, $7, $8, $9, $10, $11)",
                &[&rule_id, &rule.name, &rule.pattern, &rule.replacement, &rule.severity, &rule.description, &rule.category, &rule.rule_type, &rule.rule_config, &now, &now],
            )
            .await
        {
            Ok(_) => imported += 1,
            Err(e) => errors.push(format!("规则 '{}': {}", rule.name, e)),
        }
    }

    // 记录审计日志
    let system_user = Uuid::nil();
    write_audit_log(&client, system_user, "import_dlp_rules", &format!("导入 {} 条 DLP 规则，{} 条失败", imported, errors.len())).await;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "imported": imported,
            "errors": errors,
            "message": format!("成功导入 {} 条规则", imported),
        })),
    ))
}

// --- 客户端审计事件上报 ---

#[derive(Debug, Deserialize)]
struct ReportAuditEventRequest {
    action: String,
    details: String,
    user_id: Option<String>,
}

async fn report_audit_event(
    State(state): State<AppState>,
    Json(payload): Json<ReportAuditEventRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>)> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let user_id = payload.user_id
        .and_then(|s| Uuid::parse_str(&s).ok())
        .unwrap_or_else(Uuid::nil);

    let id = Uuid::new_v4();
    let now = chrono::Utc::now();

    client
        .execute(
            "INSERT INTO audit_logs (id, user_id, action, details, created_at) VALUES ($1, $2, $3, $4, $5)",
            &[&id, &user_id, &payload.action, &payload.details, &now],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(json!({ "id": id, "created_at": now })),
    ))
}

async fn get_sensitive_operations(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let rows = client
        .query(
            "SELECT id, operation_type, requires_approval, created_at FROM sensitive_operation_rules",
            &[],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let operations: Vec<_> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<_, Uuid>(0),
                "operation_type": r.get::<_, String>(1),
                "requires_approval": r.get::<_, bool>(2),
                "created_at": r.get::<_, chrono::DateTime<chrono::Utc>>(3),
            })
        })
        .collect();

    Ok(Json(json!({ "operations": operations })))
}

// ============================================================================
// 字典管理 (Dictionary Management)
// ============================================================================

async fn get_dictionaries(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let rows = client
        .query(
            "SELECT id, name, description, keywords, keyword_count, created_at, updated_at
             FROM dlp_dictionaries ORDER BY created_at DESC",
            &[],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let dictionaries: Vec<_> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<_, Uuid>(0),
                "name": r.get::<_, String>(1),
                "description": r.get::<_, Option<String>>(2),
                "keywords": r.get::<_, Vec<String>>(3),
                "keyword_count": r.get::<_, i32>(4),
                "created_at": r.get::<_, chrono::DateTime<chrono::Utc>>(5),
                "updated_at": r.get::<_, chrono::DateTime<chrono::Utc>>(6),
            })
        })
        .collect();

    Ok(Json(json!({ "dictionaries": dictionaries })))
}

async fn get_dictionary(
    State(state): State<AppState>,
    Path(dict_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let row = client
        .query_opt(
            "SELECT id, name, description, keywords, keyword_count, created_at, updated_at
             FROM dlp_dictionaries WHERE id = $1",
            &[&dict_id],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .ok_or(Error::NotFound("Dictionary not found".to_string()))?;

    Ok(Json(json!({
        "id": row.get::<_, Uuid>(0),
        "name": row.get::<_, String>(1),
        "description": row.get::<_, Option<String>>(2),
        "keywords": row.get::<_, Vec<String>>(3),
        "keyword_count": row.get::<_, i32>(4),
        "created_at": row.get::<_, chrono::DateTime<chrono::Utc>>(5),
        "updated_at": row.get::<_, chrono::DateTime<chrono::Utc>>(6),
    })))
}

async fn create_dictionary(
    State(state): State<AppState>,
    Json(payload): Json<CreateDictionaryRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>)> {
    if payload.name.trim().is_empty() {
        return Err(Error::Validation("字典名称不能为空".to_string()));
    }
    if payload.keywords.is_empty() {
        return Err(Error::Validation("关键字列表不能为空".to_string()));
    }

    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    // 去重并过滤空字符串
    let keywords: Vec<String> = payload.keywords.iter()
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    let keyword_count = keywords.len() as i32;
    let dict_id = Uuid::new_v4();
    let now = chrono::Utc::now();

    client
        .execute(
            "INSERT INTO dlp_dictionaries (id, name, description, keywords, keyword_count, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
            &[&dict_id, &payload.name, &payload.description, &keywords, &keyword_count, &now, &now],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let system_user = Uuid::nil();
    write_audit_log(&client, system_user, "create_dictionary", &format!("创建字典: {} ({} 个关键字)", payload.name, keyword_count)).await;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": dict_id,
            "name": payload.name,
            "description": payload.description,
            "keywords": keywords,
            "keyword_count": keyword_count,
            "created_at": now,
            "updated_at": now,
        })),
    ))
}

async fn update_dictionary(
    State(state): State<AppState>,
    Path(dict_id): Path<Uuid>,
    Json(payload): Json<UpdateDictionaryRequest>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let existing = client
        .query_opt("SELECT id FROM dlp_dictionaries WHERE id = $1", &[&dict_id])
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    if existing.is_none() {
        return Err(Error::NotFound("Dictionary not found".to_string()));
    }

    let now = chrono::Utc::now();
    let mut updates = vec!["updated_at = $1".to_string()];
    let mut params: Vec<Box<dyn tokio_postgres::types::ToSql + Sync + Send>> = vec![Box::new(now)];
    let mut param_count = 2;

    if let Some(ref name) = payload.name {
        updates.push(format!("name = ${}", param_count));
        params.push(Box::new(name.clone()));
        param_count += 1;
    }

    if let Some(ref description) = payload.description {
        updates.push(format!("description = ${}", param_count));
        params.push(Box::new(description.clone()));
        param_count += 1;
    }

    if let Some(ref keywords) = payload.keywords {
        let deduped: Vec<String> = keywords.iter()
            .map(|k| k.trim().to_string())
            .filter(|k| !k.is_empty())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        let count = deduped.len() as i32;
        updates.push(format!("keywords = ${}", param_count));
        params.push(Box::new(deduped));
        param_count += 1;
        updates.push(format!("keyword_count = ${}", param_count));
        params.push(Box::new(count));
        param_count += 1;
    }

    params.push(Box::new(dict_id));

    let param_refs: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = params.iter().map(|p| p.as_ref() as &(dyn tokio_postgres::types::ToSql + Sync)).collect();

    let query = format!(
        "UPDATE dlp_dictionaries SET {} WHERE id = ${} RETURNING id, name, description, keywords, keyword_count, created_at, updated_at",
        updates.join(", "),
        param_count
    );

    let row = client
        .query_one(&query, &param_refs)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let system_user = Uuid::nil();
    write_audit_log(&client, system_user, "update_dictionary", &format!("更新字典: {}", row.get::<_, String>(1))).await;

    Ok(Json(json!({
        "id": row.get::<_, Uuid>(0),
        "name": row.get::<_, String>(1),
        "description": row.get::<_, Option<String>>(2),
        "keywords": row.get::<_, Vec<String>>(3),
        "keyword_count": row.get::<_, i32>(4),
        "created_at": row.get::<_, chrono::DateTime<chrono::Utc>>(5),
        "updated_at": row.get::<_, chrono::DateTime<chrono::Utc>>(6),
    })))
}

async fn delete_dictionary(
    State(state): State<AppState>,
    Path(dict_id): Path<Uuid>,
) -> Result<StatusCode> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let result = client
        .execute("DELETE FROM dlp_dictionaries WHERE id = $1", &[&dict_id])
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    if result == 0 {
        return Err(Error::NotFound("Dictionary not found".to_string()));
    }

    let system_user = Uuid::nil();
    write_audit_log(&client, system_user, "delete_dictionary", &format!("删除字典: {}", dict_id)).await;

    Ok(StatusCode::NO_CONTENT)
}


// Role handlers
async fn get_roles(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let rows = client
        .query(
            "SELECT r.id, r.name, r.description, r.created_at, r.updated_at,
                    COUNT(DISTINCT ur.user_id) as user_count,
                    COUNT(DISTINCT rp.permission_id) as permission_count
             FROM roles r
             LEFT JOIN user_roles ur ON r.id = ur.role_id
             LEFT JOIN role_permissions rp ON r.id = rp.role_id
             GROUP BY r.id, r.name, r.description, r.created_at, r.updated_at
             ORDER BY r.created_at DESC",
            &[],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let roles: Vec<_> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<_, Uuid>(0),
                "name": r.get::<_, String>(1),
                "description": r.get::<_, Option<String>>(2),
                "created_at": r.get::<_, chrono::DateTime<chrono::Utc>>(3),
                "updated_at": r.get::<_, chrono::DateTime<chrono::Utc>>(4),
                "user_count": r.get::<_, i64>(5),
                "permission_count": r.get::<_, i64>(6),
            })
        })
        .collect();

    Ok(Json(json!({ "roles": roles })))
}

async fn get_role(
    State(state): State<AppState>,
    Path(role_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    // Get role info
    let role_row = client
        .query_opt(
            "SELECT id, name, description, created_at, updated_at FROM roles WHERE id = $1",
            &[&role_id],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .ok_or(Error::NotFound("Role not found".to_string()))?;

    // Get role permissions
    let permission_rows = client
        .query(
            "SELECT p.id, p.name, p.description, p.resource, p.action, p.created_at
             FROM permissions p
             INNER JOIN role_permissions rp ON p.id = rp.permission_id
             WHERE rp.role_id = $1
             ORDER BY p.resource, p.action",
            &[&role_id],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let permissions: Vec<_> = permission_rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<_, Uuid>(0),
                "name": r.get::<_, String>(1),
                "description": r.get::<_, Option<String>>(2),
                "resource": r.get::<_, String>(3),
                "action": r.get::<_, String>(4),
                "created_at": r.get::<_, chrono::DateTime<chrono::Utc>>(5),
            })
        })
        .collect();

    // Get user count
    let user_count_row = client
        .query_one(
            "SELECT COUNT(*) FROM user_roles WHERE role_id = $1",
            &[&role_id],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let user_count: i64 = user_count_row.get(0);

    Ok(Json(json!({
        "id": role_row.get::<_, Uuid>(0),
        "name": role_row.get::<_, String>(1),
        "description": role_row.get::<_, Option<String>>(2),
        "created_at": role_row.get::<_, chrono::DateTime<chrono::Utc>>(3),
        "updated_at": role_row.get::<_, chrono::DateTime<chrono::Utc>>(4),
        "permissions": permissions,
        "user_count": user_count,
    })))
}

async fn create_role(
    State(state): State<AppState>,
    Json(payload): Json<CreateRoleRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>)> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    // Check if role already exists
    let existing = client
        .query_opt("SELECT id FROM roles WHERE name = $1", &[&payload.name])
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    if existing.is_some() {
        return Err(Error::Conflict("Role already exists".to_string()));
    }

    let role_id = Uuid::new_v4();
    let now = chrono::Utc::now();

    client
        .execute(
            "INSERT INTO roles (id, name, description, created_at, updated_at) 
             VALUES ($1, $2, $3, $4, $5)",
            &[&role_id, &payload.name, &payload.description, &now, &now],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": role_id,
            "name": payload.name,
            "description": payload.description,
            "created_at": now,
            "updated_at": now,
        })),
    ))
}

async fn update_role(
    State(state): State<AppState>,
    Path(role_id): Path<Uuid>,
    Json(payload): Json<UpdateRoleRequest>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    // Check if role exists
    let existing = client
        .query_opt("SELECT id FROM roles WHERE id = $1", &[&role_id])
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    if existing.is_none() {
        return Err(Error::NotFound("Role not found".to_string()));
    }

    let now = chrono::Utc::now();
    let mut updates = vec!["updated_at = $1".to_string()];
    let mut params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = vec![&now];
    let mut param_count = 2;

    if let Some(ref name) = payload.name {
        updates.push(format!("name = ${}", param_count));
        params.push(name);
        param_count += 1;
    }

    if let Some(ref description) = payload.description {
        updates.push(format!("description = ${}", param_count));
        params.push(description);
        param_count += 1;
    }

    params.push(&role_id);

    let query = format!(
        "UPDATE roles SET {} WHERE id = ${} RETURNING id, name, description, created_at, updated_at",
        updates.join(", "),
        param_count
    );

    let row = client
        .query_one(&query, &params)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    Ok(Json(json!({
        "id": row.get::<_, Uuid>(0),
        "name": row.get::<_, String>(1),
        "description": row.get::<_, Option<String>>(2),
        "created_at": row.get::<_, chrono::DateTime<chrono::Utc>>(3),
        "updated_at": row.get::<_, chrono::DateTime<chrono::Utc>>(4),
    })))
}

async fn delete_role(
    State(state): State<AppState>,
    Path(role_id): Path<Uuid>,
) -> Result<StatusCode> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let result = client
        .execute("DELETE FROM roles WHERE id = $1", &[&role_id])
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    if result == 0 {
        return Err(Error::NotFound("Role not found".to_string()));
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn assign_permissions(
    State(state): State<AppState>,
    Path(role_id): Path<Uuid>,
    Json(payload): Json<AssignPermissionsRequest>,
) -> Result<StatusCode> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    // Check if role exists
    let existing = client
        .query_opt("SELECT id FROM roles WHERE id = $1", &[&role_id])
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    if existing.is_none() {
        return Err(Error::NotFound("Role not found".to_string()));
    }

    // Delete existing permissions
    client
        .execute("DELETE FROM role_permissions WHERE role_id = $1", &[&role_id])
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    // Insert new permissions
    let now = chrono::Utc::now();
    for permission_id in payload.permission_ids {
        client
            .execute(
                "INSERT INTO role_permissions (role_id, permission_id, created_at) VALUES ($1, $2, $3)",
                &[&role_id, &permission_id, &now],
            )
            .await
            .map_err(|e| Error::Database(e.to_string()))?;
    }

    Ok(StatusCode::NO_CONTENT)
}

// Permission handlers
async fn get_permissions(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let rows = client
        .query(
            "SELECT id, name, description, resource, action, created_at
             FROM permissions
             ORDER BY resource, action",
            &[],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let permissions: Vec<_> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<_, Uuid>(0),
                "name": r.get::<_, String>(1),
                "description": r.get::<_, Option<String>>(2),
                "resource": r.get::<_, String>(3),
                "action": r.get::<_, String>(4),
                "created_at": r.get::<_, chrono::DateTime<chrono::Utc>>(5),
            })
        })
        .collect();

    // Group permissions by resource
    let mut grouped: std::collections::HashMap<String, Vec<serde_json::Value>> = std::collections::HashMap::new();
    for permission in permissions {
        let resource = permission["resource"].as_str().unwrap().to_string();
        grouped.entry(resource).or_insert_with(Vec::new).push(permission);
    }

    Ok(Json(json!({ "permissions": grouped })))
}

// User role handlers
async fn get_user_roles(
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    // Check if user exists
    let user_exists = client
        .query_opt("SELECT id FROM users WHERE id = $1", &[&user_id])
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    if user_exists.is_none() {
        return Err(Error::UserNotFound);
    }

    // Get user roles
    let rows = client
        .query(
            "SELECT r.id, r.name, r.description
             FROM roles r
             INNER JOIN user_roles ur ON r.id = ur.role_id
             WHERE ur.user_id = $1
             ORDER BY r.name",
            &[&user_id],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let roles: Vec<_> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<_, Uuid>(0),
                "name": r.get::<_, String>(1),
                "description": r.get::<_, Option<String>>(2),
            })
        })
        .collect();

    Ok(Json(json!({ "roles": roles })))
}

async fn assign_user_roles(
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
    Json(payload): Json<AssignPermissionsRequest>,
) -> Result<StatusCode> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    // Check if user exists
    let user_exists = client
        .query_opt("SELECT id FROM users WHERE id = $1", &[&user_id])
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    if user_exists.is_none() {
        return Err(Error::UserNotFound);
    }

    // Delete existing user roles
    client
        .execute("DELETE FROM user_roles WHERE user_id = $1", &[&user_id])
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    // Insert new user roles
    let now = chrono::Utc::now();
    for role_id in payload.permission_ids {
        // Verify role exists
        let role_exists = client
            .query_opt("SELECT id FROM roles WHERE id = $1", &[&role_id])
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        if role_exists.is_none() {
            return Err(Error::NotFound(format!("Role {} not found", role_id)));
        }

        client
            .execute(
                "INSERT INTO user_roles (user_id, role_id, created_at) VALUES ($1, $2, $3)",
                &[&user_id, &role_id, &now],
            )
            .await
            .map_err(|e| Error::Database(e.to_string()))?;
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn create_user(
    State(state): State<AppState>,
    Json(payload): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>)> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    // Check if user already exists
    let existing = client
        .query_opt("SELECT id FROM users WHERE username = $1", &[&payload.username])
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    if existing.is_some() {
        return Err(Error::UserExists);
    }

    let auth = AuthManager::new(std::env::var("JWT_SECRET").unwrap_or_else(|_| "secret".to_string()));
    let password_hash = auth.hash_password(&payload.password)?;

    let user_id = Uuid::new_v4();
    let now = chrono::Utc::now();

    client
        .execute(
            "INSERT INTO users (id, username, email, password_hash, created_at, updated_at) 
             VALUES ($1, $2, $3, $4, $5, $6)",
            &[&user_id, &payload.username, &payload.email, &password_hash, &now, &now],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": user_id,
            "username": payload.username,
            "email": payload.email,
            "created_at": now
        })),
    ))
}
