use crate::auth::AuthManager;
use crate::error::{Error, Result};
use crate::handlers::{
    self,
    get_dlp_policies_handler, get_policies_handler, get_policy_version_handler,
    get_sensitive_ops_policies_handler,
};
use crate::models::{self, CreateUserRequest, LoginRequest, LoginResponse, RefreshTokenRequest, CreateRoleRequest, UpdateRoleRequest, AssignPermissionsRequest, CreateDlpRuleRequest, UpdateDlpRuleRequest, CreateDictionaryRequest, UpdateDictionaryRequest, UpdateUserRequest, CreateDepartmentRequest, UpdateDepartmentRequest, AuditLogExportQuery, UpdateClientConfigRequest};
use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post, put},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{debug, info, instrument};
use uuid::Uuid;

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/api/auth/register", post(register))
        .route("/api/auth/login", post(login))
        .route("/api/auth/refresh", post(refresh_token))
        .route("/api/users", get(get_users).post(create_user))
        .route("/api/users/{id}", get(get_user).put(update_user).delete(delete_user))
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
        .route("/api/audit-logs/export", get(export_audit_logs))
        .route("/api/audit-logs", get(get_audit_logs))
        .route("/api/audit-logs/report", post(report_audit_event))
        .route("/api/sensitive-operations", get(get_sensitive_operations).post(create_sensitive_operation))
        .route("/api/sensitive-operations/{id}", put(update_sensitive_operation).delete(delete_sensitive_operation))
        // 策略变更记录 API
        .route("/api/policy-changes", get(get_policy_changes))
        .route("/api/policy-changes/stats", get(get_policy_change_stats))
        // 客户端管理 API（注意：push-policy-all 必须在 {id} 之前注册）
        .route("/api/clients/push-policy-all", post(push_policy_all))
        .route("/api/clients/stats", get(get_client_stats))
        .route("/api/clients", get(get_clients))
        .route("/api/clients/{id}", get(get_client_detail).delete(delete_client_record))
        .route("/api/clients/{id}/disconnect", post(disconnect_client))
        .route("/api/clients/{id}/push-policy", post(push_policy_to_client))
        // 新增：策略查询 API（供 Desktop Client 使用）
        .route("/api/policies", get(get_policies_handler))
        .route("/api/policies/dlp", get(get_dlp_policies_handler))
        .route("/api/policies/sensitive-ops", get(get_sensitive_ops_policies_handler))
        .route("/api/policies/version", get(get_policy_version_handler))
        // 技能管理 API
        .route("/api/skills", get(get_skills))
        .route("/api/skills/{id}/enable", post(enable_skill))
        .route("/api/skills/{id}/disable", post(disable_skill))
        // 插件管理 API
        .route("/api/plugins", get(get_plugins))
        .route("/api/plugins/{id}/enable", post(enable_plugin))
        .route("/api/plugins/{id}/disable", post(disable_plugin))
        // 系统配置 API
        .route("/api/settings", get(get_settings).put(update_settings))
        // 客户端配置下发 API
        .route("/api/client-config", get(get_client_config).put(update_client_config))
        // 客户端数据上报 API（供 Desktop Client 使用）
        .route("/api/client-reports", post(post_client_reports))
        // 仪表盘 API
        .route("/api/dashboard/stats", get(get_dashboard_stats))
        .route("/api/dashboard/activity", get(get_dashboard_activity))
        .route("/api/dashboard/trends", get(get_dashboard_trends))
        // 部门管理 API
        .route("/api/departments", get(get_departments).post(create_department))
        .route("/api/departments/{id}", get(handlers::departments::get_department_detail).put(update_department).delete(delete_department))
        .route("/api/departments/{id}/members", get(handlers::departments::get_department_members))
        .route("/api/departments/{id}/model-whitelist", get(handlers::departments::get_model_whitelist).put(handlers::departments::update_model_whitelist))
        .route("/api/departments/{id}/quota-summary", get(handlers::quota::department_quota_summary))
        // 模型配置 API
        .route("/api/model-configs", get(get_model_configs).post(create_model_config))
        .route("/api/model-configs/{id}", put(update_model_config).delete(delete_model_config))
        .route("/api/model-configs/test-connection", post(test_model_connection))
        // 客户端模型列表（精简版，供 Desktop Client 拉取）
        .route("/api/client-models", get(get_client_models))
        // 告警与通知 API
        .route("/api/alert-rules", get(handlers::alerts::get_alert_rules).post(handlers::alerts::create_alert_rule))
        .route("/api/alert-rules/{id}", put(handlers::alerts::update_alert_rule).delete(handlers::alerts::delete_alert_rule))
        .route("/api/alerts", get(handlers::alerts::get_alert_events))
        .route("/api/alerts/stats", get(handlers::alerts::get_alert_stats))
        .route("/api/alerts/trigger", post(handlers::alerts::manual_trigger_alert))
        .route("/api/alerts/{id}/status", put(handlers::alerts::update_alert_event_status))
        // 对话审计 API
        .route("/api/conversations/stats", get(handlers::conversations::get_conversation_stats))
        .route("/api/conversations", get(handlers::conversations::get_conversations))
        .route("/api/conversations/{id}", get(handlers::conversations::get_conversation_detail))
        // 费用配额管理 API
        .route("/api/quota/check", post(handlers::quota::quota_check))
        .route("/api/quota/report-usage", post(handlers::quota::report_usage))
        .route("/api/quota/overview", get(handlers::quota::quota_overview))
        .route("/api/quota/config", get(handlers::quota::get_quota_config).put(handlers::quota::update_quota_config))
        .route("/api/quota/department-ranking", get(handlers::quota::department_ranking))
        .route("/api/quota/model-ranking", get(handlers::quota::model_ranking))
        .route("/api/quota/usage-records", get(handlers::quota::usage_records))
        .with_state(state)
}

async fn health_check() -> impl IntoResponse {
    Json(json!({ "status": "ok" }))
}

/// 写入审计日志的辅助函数（不阻塞主流程，失败仅打印日志）
pub async fn write_audit_log(
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
            "SELECT u.id, u.username, u.email, u.created_at, u.updated_at, u.department_id, d.name as dept_name
             FROM users u
             LEFT JOIN departments d ON u.department_id = d.id
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

        let department = match row.get::<_, Option<Uuid>>(5) {
            Some(dept_id) => json!({
                "id": dept_id,
                "name": row.get::<_, Option<String>>(6),
            }),
            None => json!(null),
        };

        users.push(json!({
            "id": user_id,
            "username": row.get::<_, String>(1),
            "email": row.get::<_, String>(2),
            "created_at": row.get::<_, chrono::DateTime<chrono::Utc>>(3),
            "updated_at": row.get::<_, chrono::DateTime<chrono::Utc>>(4),
            "roles": roles,
            "department": department,
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
            "SELECT id, name, operation_type, requires_approval, risk_level, description, enabled, approver_roles, created_at, updated_at
             FROM sensitive_operation_rules ORDER BY created_at DESC",
            &[],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let operations: Vec<_> = rows
        .iter()
        .map(|r| {
            let approver_roles: Vec<String> = r.try_get::<_, Vec<String>>(7).unwrap_or_default();
            json!({
                "id": r.get::<_, Uuid>(0),
                "name": r.get::<_, String>(1),
                "operation_type": r.get::<_, String>(2),
                "requires_approval": r.get::<_, bool>(3),
                "risk_level": r.try_get::<_, String>(4).unwrap_or_else(|_| "medium".to_string()),
                "description": r.try_get::<_, Option<String>>(5).unwrap_or(None),
                "enabled": r.try_get::<_, bool>(6).unwrap_or(true),
                "approver_roles": approver_roles,
                "created_at": r.get::<_, chrono::DateTime<chrono::Utc>>(8),
                "updated_at": r.try_get::<_, chrono::DateTime<chrono::Utc>>(9).unwrap_or_else(|_| r.get::<_, chrono::DateTime<chrono::Utc>>(8)),
            })
        })
        .collect();

    Ok(Json(json!({ "operations": operations })))
}

async fn create_sensitive_operation(
    State(state): State<AppState>,
    Json(payload): Json<models::CreateSensitiveOperationRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>)> {
    // 验证
    if payload.name.trim().is_empty() || payload.name.len() < 2 || payload.name.len() > 100 {
        return Err(Error::Validation("名称长度必须在 2-100 字符之间".to_string()));
    }
    if payload.operation_type.trim().is_empty() {
        return Err(Error::Validation("操作类型不能为空".to_string()));
    }

    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let id = Uuid::new_v4();
    let now = chrono::Utc::now();
    let risk_level = payload.risk_level.unwrap_or_else(|| "medium".to_string());
    let approver_roles = payload.approver_roles.unwrap_or_default();

    client
        .execute(
            "INSERT INTO sensitive_operation_rules (id, name, operation_type, requires_approval, risk_level, description, enabled, approver_roles, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
            &[&id, &payload.name, &payload.operation_type, &payload.requires_approval.unwrap_or(true),
              &risk_level, &payload.description, &true, &approver_roles, &now, &now],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": id,
            "name": payload.name,
            "operation_type": payload.operation_type,
            "requires_approval": payload.requires_approval.unwrap_or(true),
            "risk_level": risk_level,
            "description": payload.description,
            "enabled": true,
            "approver_roles": approver_roles,
            "created_at": now,
            "updated_at": now,
        })),
    ))
}

async fn update_sensitive_operation(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<models::UpdateSensitiveOperationRequest>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    // 检查是否存在
    let existing = client
        .query_opt("SELECT id FROM sensitive_operation_rules WHERE id = $1", &[&id])
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    if existing.is_none() {
        return Err(Error::NotFound("敏感操作规则不存在".to_string()));
    }

    // 验证名称
    if let Some(ref name) = payload.name {
        if name.trim().is_empty() || name.len() < 2 || name.len() > 100 {
            return Err(Error::Validation("名称长度必须在 2-100 字符之间".to_string()));
        }
    }

    let now = chrono::Utc::now();

    // 动态构建 UPDATE 语句
    let mut set_clauses = vec!["updated_at = $2".to_string()];
    let mut param_idx = 3u32;
    let mut params: Vec<Box<dyn tokio_postgres::types::ToSql + Sync + Send>> = vec![
        Box::new(id),
        Box::new(now),
    ];

    if let Some(ref name) = payload.name {
        set_clauses.push(format!("name = ${}", param_idx));
        params.push(Box::new(name.clone()));
        param_idx += 1;
    }
    if let Some(ref op_type) = payload.operation_type {
        set_clauses.push(format!("operation_type = ${}", param_idx));
        params.push(Box::new(op_type.clone()));
        param_idx += 1;
    }
    if let Some(requires_approval) = payload.requires_approval {
        set_clauses.push(format!("requires_approval = ${}", param_idx));
        params.push(Box::new(requires_approval));
        param_idx += 1;
    }
    if let Some(ref risk_level) = payload.risk_level {
        set_clauses.push(format!("risk_level = ${}", param_idx));
        params.push(Box::new(risk_level.clone()));
        param_idx += 1;
    }
    if let Some(ref description) = payload.description {
        set_clauses.push(format!("description = ${}", param_idx));
        params.push(Box::new(description.clone()));
        param_idx += 1;
    }
    if let Some(enabled) = payload.enabled {
        set_clauses.push(format!("enabled = ${}", param_idx));
        params.push(Box::new(enabled));
        param_idx += 1;
    }
    if let Some(ref approver_roles) = payload.approver_roles {
        set_clauses.push(format!("approver_roles = ${}", param_idx));
        params.push(Box::new(approver_roles.clone()));
        let _ = param_idx;
    }

    let sql = format!(
        "UPDATE sensitive_operation_rules SET {} WHERE id = $1",
        set_clauses.join(", ")
    );

    let params_ref: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
        params.iter().map(|p| p.as_ref() as &(dyn tokio_postgres::types::ToSql + Sync)).collect();

    client
        .execute(&sql, &params_ref)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    Ok(Json(json!({ "message": "更新成功", "updated_at": now })))
}

async fn delete_sensitive_operation(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let result = client
        .execute("DELETE FROM sensitive_operation_rules WHERE id = $1", &[&id])
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    if result == 0 {
        return Err(Error::NotFound("敏感操作规则不存在".to_string()));
    }

    Ok(Json(json!({ "message": "删除成功" })))
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

// ============================================================================
// 策略变更记录 (Policy Change Records)
// ============================================================================

#[derive(Debug, Deserialize)]
struct PolicyChangeQuery {
    /// 规则类型筛选: dlp_rule, sensitive_op, dictionary
    rule_type: Option<String>,
    /// 变更类型筛选: create, update, delete, enable, disable, import
    change_type: Option<String>,
    /// 开始日期 (ISO 8601)
    start_date: Option<String>,
    /// 结束日期 (ISO 8601)
    end_date: Option<String>,
    /// 搜索关键字（匹配规则名称或操作人）
    search: Option<String>,
    /// 每页数量，默认 20
    page_size: Option<i64>,
    /// 页码，默认 1
    page: Option<i64>,
}

async fn get_policy_changes(
    State(state): State<AppState>,
    Query(params): Query<PolicyChangeQuery>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let page_size = params.page_size.unwrap_or(20).min(100).max(1);
    let page = params.page.unwrap_or(1).max(1);
    let offset = (page - 1) * page_size;

    // 动态构建 WHERE 子句
    let mut conditions: Vec<String> = vec![];
    let mut query_params: Vec<Box<dyn tokio_postgres::types::ToSql + Sync + Send>> = vec![];
    let mut param_idx = 1u32;

    if let Some(ref rule_type) = params.rule_type {
        conditions.push(format!("rule_type = ${}", param_idx));
        query_params.push(Box::new(rule_type.clone()));
        param_idx += 1;
    }

    if let Some(ref change_type) = params.change_type {
        conditions.push(format!("change_type = ${}", param_idx));
        query_params.push(Box::new(change_type.clone()));
        param_idx += 1;
    }

    if let Some(ref start_date) = params.start_date {
        if let Ok(dt) = chrono::NaiveDate::parse_from_str(start_date, "%Y-%m-%d") {
            let start = dt.and_hms_opt(0, 0, 0).unwrap();
            let start_utc = chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(start, chrono::Utc);
            conditions.push(format!("changed_at >= ${}", param_idx));
            query_params.push(Box::new(start_utc));
            param_idx += 1;
        }
    }

    if let Some(ref end_date) = params.end_date {
        if let Ok(dt) = chrono::NaiveDate::parse_from_str(end_date, "%Y-%m-%d") {
            let end = dt.and_hms_opt(23, 59, 59).unwrap();
            let end_utc = chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(end, chrono::Utc);
            conditions.push(format!("changed_at <= ${}", param_idx));
            query_params.push(Box::new(end_utc));
            param_idx += 1;
        }
    }

    if let Some(ref search) = params.search {
        let pattern = format!("%{}%", search);
        conditions.push(format!("(rule_name ILIKE ${0} OR changed_by_name ILIKE ${0})", param_idx));
        query_params.push(Box::new(pattern));
        param_idx += 1;
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    // 查询总数
    let count_sql = format!("SELECT COUNT(*) FROM policy_change_records {}", where_clause);
    let count_params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
        query_params.iter().map(|p| p.as_ref() as &(dyn tokio_postgres::types::ToSql + Sync)).collect();
    let count_row = client
        .query_one(&count_sql, &count_params)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;
    let total: i64 = count_row.get(0);

    // 查询数据
    query_params.push(Box::new(page_size));
    let limit_idx = param_idx;
    param_idx += 1;
    query_params.push(Box::new(offset));
    let offset_idx = param_idx;

    let data_sql = format!(
        "SELECT id, rule_id, rule_type, rule_name, change_type, field_changed, old_value, new_value, changed_by, changed_by_name, changed_at, reason
         FROM policy_change_records {}
         ORDER BY changed_at DESC
         LIMIT ${} OFFSET ${}",
        where_clause, limit_idx, offset_idx
    );

    let data_params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
        query_params.iter().map(|p| p.as_ref() as &(dyn tokio_postgres::types::ToSql + Sync)).collect();
    let rows = client
        .query(&data_sql, &data_params)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let records: Vec<_> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<_, Uuid>(0),
                "rule_id": r.get::<_, Uuid>(1),
                "rule_type": r.get::<_, String>(2),
                "rule_name": r.try_get::<_, Option<String>>(3).unwrap_or(None),
                "change_type": r.get::<_, String>(4),
                "field_changed": r.try_get::<_, Option<String>>(5).unwrap_or(None),
                "old_value": r.try_get::<_, Option<serde_json::Value>>(6).unwrap_or(None),
                "new_value": r.try_get::<_, Option<serde_json::Value>>(7).unwrap_or(None),
                "changed_by": r.try_get::<_, Option<Uuid>>(8).unwrap_or(None),
                "changed_by_name": r.try_get::<_, Option<String>>(9).unwrap_or(Some("system".to_string())),
                "changed_at": r.get::<_, chrono::DateTime<chrono::Utc>>(10),
                "reason": r.try_get::<_, Option<String>>(11).unwrap_or(None),
            })
        })
        .collect();

    let total_pages = (total as f64 / page_size as f64).ceil() as i64;

    Ok(Json(json!({
        "records": records,
        "total": total,
        "page": page,
        "page_size": page_size,
        "total_pages": total_pages,
    })))
}

async fn get_policy_change_stats(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    // 按变更类型统计
    let type_rows = client
        .query(
            "SELECT change_type, COUNT(*) as count FROM policy_change_records GROUP BY change_type ORDER BY count DESC",
            &[],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let by_change_type: Vec<_> = type_rows.iter().map(|r| {
        json!({ "type": r.get::<_, String>(0), "count": r.get::<_, i64>(1) })
    }).collect();

    // 按规则类型统计
    let rule_rows = client
        .query(
            "SELECT rule_type, COUNT(*) as count FROM policy_change_records GROUP BY rule_type ORDER BY count DESC",
            &[],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let by_rule_type: Vec<_> = rule_rows.iter().map(|r| {
        json!({ "type": r.get::<_, String>(0), "count": r.get::<_, i64>(1) })
    }).collect();

    // 最近 7 天趋势
    let trend_rows = client
        .query(
            "SELECT DATE(changed_at) as date, COUNT(*) as count
             FROM policy_change_records
             WHERE changed_at >= NOW() - INTERVAL '7 days'
             GROUP BY DATE(changed_at)
             ORDER BY date",
            &[],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let trend: Vec<_> = trend_rows.iter().map(|r| {
        json!({
            "date": r.get::<_, chrono::NaiveDate>(0).to_string(),
            "count": r.get::<_, i64>(1),
        })
    }).collect();

    // 总数
    let total_row = client
        .query_one("SELECT COUNT(*) FROM policy_change_records", &[])
        .await
        .map_err(|e| Error::Database(e.to_string()))?;
    let total: i64 = total_row.get(0);

    Ok(Json(json!({
        "total": total,
        "by_change_type": by_change_type,
        "by_rule_type": by_rule_type,
        "trend_7d": trend,
    })))
}


// ============================================================================
// 客户端管理 (Client Management)
// ============================================================================

#[derive(Debug, Deserialize)]
struct ClientQuery {
    search: Option<String>,
    online: Option<bool>,
    os: Option<String>,
}

async fn get_clients(
    State(state): State<AppState>,
    Query(params): Query<ClientQuery>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let mut conditions: Vec<String> = vec![];
    let mut query_params: Vec<Box<dyn tokio_postgres::types::ToSql + Sync + Send>> = vec![];
    let mut idx = 1u32;

    if let Some(ref search) = params.search {
        let pattern = format!("%{}%", search);
        conditions.push(format!("(username ILIKE ${0} OR client_name ILIKE ${0} OR ip_address ILIKE ${0})", idx));
        query_params.push(Box::new(pattern));
        idx += 1;
    }

    if let Some(online) = params.online {
        conditions.push(format!("online = ${}", idx));
        query_params.push(Box::new(online));
        idx += 1;
    }

    if let Some(ref os) = params.os {
        conditions.push(format!("os ILIKE ${}", idx));
        query_params.push(Box::new(format!("%{}%", os)));
        let _ = idx;
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let sql = format!(
        "SELECT id, user_id, username, client_name, version, os, ip_address, last_activity, online, policy_version, registered_at, updated_at
         FROM registered_clients {} ORDER BY last_activity DESC",
        where_clause
    );

    let param_refs: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
        query_params.iter().map(|p| p.as_ref() as &(dyn tokio_postgres::types::ToSql + Sync)).collect();

    let rows = client.query(&sql, &param_refs).await
        .map_err(|e| Error::Database(e.to_string()))?;

    let clients: Vec<_> = rows.iter().map(|r| {
        json!({
            "id": r.get::<_, Uuid>(0),
            "user_id": r.try_get::<_, Option<Uuid>>(1).unwrap_or(None),
            "username": r.try_get::<_, Option<String>>(2).unwrap_or(None),
            "client_name": r.try_get::<_, Option<String>>(3).unwrap_or(None),
            "version": r.try_get::<_, Option<String>>(4).unwrap_or(None),
            "os": r.try_get::<_, Option<String>>(5).unwrap_or(None),
            "ip_address": r.try_get::<_, Option<String>>(6).unwrap_or(None),
            "last_activity": r.get::<_, chrono::DateTime<chrono::Utc>>(7),
            "online": r.get::<_, bool>(8),
            "policy_version": r.try_get::<_, Option<String>>(9).unwrap_or(None),
            "registered_at": r.get::<_, chrono::DateTime<chrono::Utc>>(10),
            "updated_at": r.get::<_, chrono::DateTime<chrono::Utc>>(11),
        })
    }).collect();

    Ok(Json(json!({ "clients": clients })))
}

async fn get_client_detail(
    State(state): State<AppState>,
    Path(client_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let row = client
        .query_opt(
            "SELECT id, user_id, username, client_name, version, os, ip_address, last_activity, online, policy_version, registered_at, updated_at
             FROM registered_clients WHERE id = $1",
            &[&client_id],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .ok_or(Error::NotFound("客户端不存在".to_string()))?;

    Ok(Json(json!({
        "id": row.get::<_, Uuid>(0),
        "user_id": row.try_get::<_, Option<Uuid>>(1).unwrap_or(None),
        "username": row.try_get::<_, Option<String>>(2).unwrap_or(None),
        "client_name": row.try_get::<_, Option<String>>(3).unwrap_or(None),
        "version": row.try_get::<_, Option<String>>(4).unwrap_or(None),
        "os": row.try_get::<_, Option<String>>(5).unwrap_or(None),
        "ip_address": row.try_get::<_, Option<String>>(6).unwrap_or(None),
        "last_activity": row.get::<_, chrono::DateTime<chrono::Utc>>(7),
        "online": row.get::<_, bool>(8),
        "policy_version": row.try_get::<_, Option<String>>(9).unwrap_or(None),
        "registered_at": row.get::<_, chrono::DateTime<chrono::Utc>>(10),
        "updated_at": row.get::<_, chrono::DateTime<chrono::Utc>>(11),
    })))
}

async fn delete_client_record(
    State(state): State<AppState>,
    Path(client_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let result = client
        .execute("DELETE FROM registered_clients WHERE id = $1", &[&client_id])
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    if result == 0 {
        return Err(Error::NotFound("客户端不存在".to_string()));
    }

    Ok(Json(json!({ "message": "删除成功" })))
}

async fn get_client_stats(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let total_row = client.query_one("SELECT COUNT(*) FROM registered_clients", &[]).await
        .map_err(|e| Error::Database(e.to_string()))?;
    let total: i64 = total_row.get(0);

    let online_row = client.query_one("SELECT COUNT(*) FROM registered_clients WHERE online = true", &[]).await
        .map_err(|e| Error::Database(e.to_string()))?;
    let online: i64 = online_row.get(0);

    let os_rows = client.query(
        "SELECT COALESCE(os, 'Unknown') as os, COUNT(*) as count FROM registered_clients GROUP BY os ORDER BY count DESC",
        &[],
    ).await.map_err(|e| Error::Database(e.to_string()))?;

    let by_os: Vec<_> = os_rows.iter().map(|r| {
        json!({ "os": r.get::<_, String>(0), "count": r.get::<_, i64>(1) })
    }).collect();

    let version_rows = client.query(
        "SELECT COALESCE(version, 'Unknown') as version, COUNT(*) as count FROM registered_clients GROUP BY version ORDER BY count DESC",
        &[],
    ).await.map_err(|e| Error::Database(e.to_string()))?;

    let by_version: Vec<_> = version_rows.iter().map(|r| {
        json!({ "version": r.get::<_, String>(0), "count": r.get::<_, i64>(1) })
    }).collect();

    Ok(Json(json!({
        "total": total,
        "online": online,
        "offline": total - online,
        "by_os": by_os,
        "by_version": by_version,
    })))
}

// ==================== 技能管理 API ====================
// 代理到 IronClaw Web Gateway 的 /api/skills 端点

async fn get_skills(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>> {
    let url = format!("{}/api/skills", state.gateway_url);
    match state.http_client.get(&url).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                let body: serde_json::Value = resp.json().await
                    .map_err(|e| Error::Internal(format!("解析 Gateway 响应失败: {}", e)))?;
                Ok(Json(body))
            } else {
                // Gateway 返回错误，返回空列表
                Ok(Json(json!({ "skills": [], "count": 0 })))
            }
        }
        Err(_) => {
            // Gateway 不可用，从本地数据库回退查询
            let client = state.db_pool.get().await
                .map_err(|e| Error::Database(e.to_string()))?;

            let rows = client.query(
                "SELECT id, name, COALESCE(description, '') as description, version, COALESCE(author, '') as author, enabled, created_at, updated_at FROM skills ORDER BY name ASC",
                &[],
            ).await.map_err(|e| Error::Database(e.to_string()))?;

            let skills: Vec<serde_json::Value> = rows.iter().map(|r| {
                json!({
                    "id": r.get::<_, uuid::Uuid>(0).to_string(),
                    "name": r.get::<_, String>(1),
                    "description": r.get::<_, String>(2),
                    "version": r.get::<_, String>(3),
                    "author": r.get::<_, String>(4),
                    "enabled": r.get::<_, bool>(5),
                    "created_at": r.get::<_, chrono::DateTime<chrono::Utc>>(6).to_rfc3339(),
                    "updated_at": r.get::<_, chrono::DateTime<chrono::Utc>>(7).to_rfc3339(),
                })
            }).collect();

            Ok(Json(json!({ "skills": skills, "count": skills.len() })))
        }
    }
}

// ==================== 插件管理 API ====================
// 代理到 IronClaw Web Gateway 的 /api/extensions 端点

async fn get_plugins(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>> {
    let url = format!("{}/api/extensions", state.gateway_url);
    match state.http_client.get(&url).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                let body: serde_json::Value = resp.json().await
                    .map_err(|e| Error::Internal(format!("解析 Gateway 响应失败: {}", e)))?;
                // 将 extensions 格式转换为前端期望的 plugins 格式
                let extensions = body.get("extensions").cloned().unwrap_or(json!([]));
                let plugins: Vec<serde_json::Value> = if let Some(arr) = extensions.as_array() {
                    arr.iter().map(|ext| {
                        json!({
                            "id": ext.get("name").and_then(|n| n.as_str()).unwrap_or(""),
                            "name": ext.get("display_name").and_then(|n| n.as_str())
                                .or_else(|| ext.get("name").and_then(|n| n.as_str()))
                                .unwrap_or(""),
                            "description": ext.get("description").and_then(|n| n.as_str()).unwrap_or(""),
                            "version": ext.get("version").and_then(|n| n.as_str()).unwrap_or("1.0.0"),
                            "author": "",
                            "enabled": ext.get("active").and_then(|n| n.as_bool()).unwrap_or(false),
                            "created_at": chrono::Utc::now().to_rfc3339(),
                            "updated_at": chrono::Utc::now().to_rfc3339(),
                        })
                    }).collect()
                } else {
                    vec![]
                };
                Ok(Json(json!({ "plugins": plugins })))
            } else {
                Ok(Json(json!({ "plugins": [] })))
            }
        }
        Err(_) => {
            // Gateway 不可用，从本地数据库回退查询
            let client = state.db_pool.get().await
                .map_err(|e| Error::Database(e.to_string()))?;

            let rows = client.query(
                "SELECT id, name, COALESCE(description, '') as description, version, COALESCE(author, '') as author, enabled, created_at, updated_at FROM plugins ORDER BY name ASC",
                &[],
            ).await.map_err(|e| Error::Database(e.to_string()))?;

            let plugins: Vec<serde_json::Value> = rows.iter().map(|r| {
                json!({
                    "id": r.get::<_, uuid::Uuid>(0).to_string(),
                    "name": r.get::<_, String>(1),
                    "description": r.get::<_, String>(2),
                    "version": r.get::<_, String>(3),
                    "author": r.get::<_, String>(4),
                    "enabled": r.get::<_, bool>(5),
                    "created_at": r.get::<_, chrono::DateTime<chrono::Utc>>(6).to_rfc3339(),
                    "updated_at": r.get::<_, chrono::DateTime<chrono::Utc>>(7).to_rfc3339(),
                })
            }).collect();

            Ok(Json(json!({ "plugins": plugins })))
        }
    }
}

// ==================== 系统配置 API ====================

async fn get_settings(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let rows = client.query(
        "SELECT key, value FROM system_settings",
        &[],
    ).await.map_err(|e| Error::Database(e.to_string()))?;

    let mut config = json!({
        "dlp_enabled": true,
        "dlp_scan_timeout_ms": 5000,
        "dlp_fail_open": false,
        "audit_retention_days": 90,
        "audit_enabled": true,
        "client_heartbeat_interval_s": 30,
        "client_offline_threshold_s": 120,
        "policy_sync_interval_s": 300,
        "policy_auto_push": true,
    });

    for row in &rows {
        let key: String = row.get(0);
        let value: serde_json::Value = row.get(1);
        config[&key] = value;
    }

    Ok(Json(config))
}

async fn update_settings(
    State(state): State<AppState>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let valid_keys = [
        "dlp_enabled", "dlp_scan_timeout_ms", "dlp_fail_open",
        "audit_retention_days", "audit_enabled",
        "client_heartbeat_interval_s", "client_offline_threshold_s",
        "policy_sync_interval_s", "policy_auto_push",
    ];

    if let Some(obj) = payload.as_object() {
        for (key, value) in obj {
            if valid_keys.contains(&key.as_str()) {
                client.execute(
                    "INSERT INTO system_settings (key, value, updated_at) VALUES ($1, $2, NOW()) ON CONFLICT (key) DO UPDATE SET value = $2, updated_at = NOW()",
                    &[&key, &value],
                ).await.map_err(|e| Error::Database(e.to_string()))?;
            }
        }
    }

    Ok(Json(json!({ "message": "配置保存成功" })))
}

// =========================================================================
// 客户端配置下发 API
// =========================================================================

/// 客户端配置响应。
#[derive(Debug, Serialize, Deserialize)]
struct ClientConfigResponse {
    llm_backend: Option<String>,
    llm_api_key: Option<String>,
    llm_model: Option<String>,
    llm_base_url: Option<String>,
    safety_enabled: Option<bool>,
    skills_enabled: Option<bool>,
    extensions_enabled: Option<bool>,
    max_cost_per_day_cents: Option<i64>,
    config_version: i64,
    updated_at: String,
}

/// GET /api/client-config — 返回客户端应使用的配置。
///
/// 查找顺序：
/// 1. 特定客户端配置（通过 `client_id` 查询参数）
/// 2. 全局默认配置（`client_id IS NULL`）
/// 3. 如果都没有，返回空配置
#[instrument(skip(state))]
async fn get_client_config(
    State(state): State<AppState>,
    Query(params): Query<ClientConfigQuery>,
) -> Result<Json<ClientConfigResponse>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    // 尝试查找特定客户端配置，否则使用全局默认
    let row = if let Some(ref cid) = params.client_id {
        let uuid = Uuid::parse_str(cid)
            .map_err(|_| Error::Validation("Invalid client_id format".into()))?;
        client.query_opt(
            "SELECT llm_backend, llm_api_key, llm_model, llm_base_url, \
             safety_enabled, skills_enabled, extensions_enabled, \
             max_cost_per_day_cents, config_version, updated_at \
             FROM client_configs WHERE client_id = $1",
            &[&uuid],
        ).await.map_err(|e| Error::Database(e.to_string()))?
    } else {
        None
    };

    // 如果没有特定配置，查找全局默认
    let row = match row {
        Some(r) => Some(r),
        None => client.query_opt(
            "SELECT llm_backend, llm_api_key, llm_model, llm_base_url, \
             safety_enabled, skills_enabled, extensions_enabled, \
             max_cost_per_day_cents, config_version, updated_at \
             FROM client_configs WHERE client_id IS NULL",
            &[],
        ).await.map_err(|e| Error::Database(e.to_string()))?,
    };

    let response = match row {
        Some(r) => {
            let updated_at: DateTime<Utc> = r.get(9);
            ClientConfigResponse {
                llm_backend: r.get(0),
                llm_api_key: r.get::<_, Option<String>>(1).map(|k| mask_api_key(&k)),
                llm_model: r.get(2),
                llm_base_url: r.get(3),
                safety_enabled: r.get(4),
                skills_enabled: r.get(5),
                extensions_enabled: r.get(6),
                max_cost_per_day_cents: r.get(7),
                config_version: r.get(8),
                updated_at: updated_at.to_rfc3339(),
            }
        }
        None => ClientConfigResponse {
            llm_backend: None,
            llm_api_key: None,
            llm_model: None,
            llm_base_url: None,
            safety_enabled: None,
            skills_enabled: None,
            extensions_enabled: None,
            max_cost_per_day_cents: None,
            config_version: 0,
            updated_at: Utc::now().to_rfc3339(),
        },
    };

    debug!(version = response.config_version, "Client config served");
    Ok(Json(response))
}

#[derive(Debug, Deserialize)]
struct ClientConfigQuery {
    client_id: Option<String>,
}

// =========================================================================
// 客户端数据上报 API
// =========================================================================

/// 客户端上报事件。
#[derive(Debug, Deserialize)]
struct ClientReportPayload {
    #[serde(rename = "type")]
    report_type: String,
    #[serde(flatten)]
    data: serde_json::Value,
}

/// POST /api/client-reports — 接收客户端上报的事件。
///
/// 请求体为 JSON 数组，每个元素包含 `type` 字段标识事件类型。
/// 支持的类型：`audit_log`, `dlp_event`, `usage_stats`, `health_status`。
#[instrument(skip(state, payload))]
async fn post_client_reports(
    State(state): State<AppState>,
    Json(payload): Json<Vec<ClientReportPayload>>,
) -> Result<(StatusCode, Json<serde_json::Value>)> {
    if payload.is_empty() {
        return Ok((
            StatusCode::OK,
            Json(json!({ "received": 0 })),
        ));
    }

    if payload.len() > 1000 {
        return Err(Error::Validation("Too many reports in batch (max 1000)".into()));
    }

    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let now = Utc::now();
    let mut inserted = 0u64;

    for report in &payload {
        let valid_types = ["audit_log", "dlp_event", "usage_stats", "health_status", "conversation"];
        if !valid_types.contains(&report.report_type.as_str()) {
            tracing::warn!(report_type = %report.report_type, "Unknown report type, skipping");
            continue;
        }

        // conversation 类型走专用处理逻辑（幂等写入 conversations 表）
        if report.report_type == "conversation" {
            match serde_json::from_value::<crate::models::ConversationReportPayload>(report.data.clone()) {
                Ok(conv_payload) => {
                    match handlers::conversations::ingest_conversation(&client, &conv_payload).await {
                        Ok(true) => { inserted += 1; }
                        Ok(false) => { /* 幂等跳过 */ }
                        Err(e) => { tracing::warn!(error = %e, "Failed to ingest conversation"); }
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Invalid conversation payload");
                }
            }
            continue;
        }

        let id = Uuid::new_v4();
        client.execute(
            "INSERT INTO client_reports (id, report_type, payload, received_at) \
             VALUES ($1, $2, $3, $4)",
            &[&id, &report.report_type, &report.data, &now],
        ).await.map_err(|e| Error::Database(e.to_string()))?;

        inserted += 1;
    }

    info!(count = inserted, "Client reports received");

    Ok((
        StatusCode::CREATED,
        Json(json!({ "received": inserted })),
    ))
}



// ============================================================================
// 辅助函数
// ============================================================================

/// API Key 脱敏：保留前 4 位，其余替换为 ****
fn mask_api_key(key: &str) -> String {
    if key.len() <= 4 {
        "****".to_string()
    } else {
        format!("{}****", &key[..4])
    }
}

/// CSV 字段转义：包含逗号、换行或双引号时用双引号包裹
fn escape_csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('\n') || s.contains('\r') || s.contains('"') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// 邮箱格式验证
fn is_valid_email(email: &str) -> bool {
    let re = regex::Regex::new(r"^[^@\s]+@[^@\s]+\.[^@\s]+$").unwrap();
    re.is_match(email)
}

// ============================================================================
// 用户编辑 (PUT /api/users/{id})
// ============================================================================

async fn update_user(
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
    Json(payload): Json<UpdateUserRequest>,
) -> Result<Json<serde_json::Value>> {
    // 验证 email 格式
    if let Some(ref email) = payload.email {
        if !is_valid_email(email) {
            return Err(Error::Validation("邮箱格式不正确".to_string()));
        }
    }

    // 验证密码长度
    if let Some(ref password) = payload.password {
        if password.len() < 8 || password.len() > 128 {
            return Err(Error::Validation("密码长度必须在 8-128 字符之间".to_string()));
        }
    }

    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    // 检查用户是否存在
    let existing = client
        .query_opt("SELECT id FROM users WHERE id = $1", &[&user_id])
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    if existing.is_none() {
        return Err(Error::UserNotFound);
    }

    let now = chrono::Utc::now();
    let mut set_clauses = vec!["updated_at = $1".to_string()];
    let mut params: Vec<Box<dyn tokio_postgres::types::ToSql + Sync + Send>> = vec![Box::new(now)];
    let mut param_idx = 2u32;
    let mut changed_fields: Vec<String> = vec![];

    if let Some(ref email) = payload.email {
        set_clauses.push(format!("email = ${}", param_idx));
        params.push(Box::new(email.clone()));
        param_idx += 1;
        changed_fields.push("email".to_string());
    }

    if let Some(ref password) = payload.password {
        let auth = AuthManager::new(
            std::env::var("JWT_SECRET").unwrap_or_else(|_| "secret".to_string()),
        );
        let password_hash = auth.hash_password(password)?;
        set_clauses.push(format!("password_hash = ${}", param_idx));
        params.push(Box::new(password_hash));
        param_idx += 1;
        changed_fields.push("password".to_string());
    }

    if let Some(ref dept_id_opt) = payload.department_id {
        set_clauses.push(format!("department_id = ${}", param_idx));
        params.push(Box::new(*dept_id_opt));
        param_idx += 1;
        changed_fields.push("department_id".to_string());
    }

    // 如果没有任何字段需要更新
    if changed_fields.is_empty() {
        return Err(Error::Validation("至少需要提供一个要更新的字段".to_string()));
    }

    params.push(Box::new(user_id));

    let sql = format!(
        "UPDATE users SET {} WHERE id = ${} RETURNING id, username, email, created_at, updated_at",
        set_clauses.join(", "),
        param_idx
    );

    let param_refs: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
        params.iter().map(|p| p.as_ref() as &(dyn tokio_postgres::types::ToSql + Sync)).collect();

    let row = client
        .query_one(&sql, &param_refs)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    // 写入审计日志
    let details = format!("更新用户 {}，修改字段: {}", row.get::<_, String>(1), changed_fields.join(", "));
    write_audit_log(&client, user_id, "update_user", &details).await;

    Ok(Json(json!({
        "id": row.get::<_, Uuid>(0),
        "username": row.get::<_, String>(1),
        "email": row.get::<_, String>(2),
        "created_at": row.get::<_, chrono::DateTime<chrono::Utc>>(3),
        "updated_at": row.get::<_, chrono::DateTime<chrono::Utc>>(4),
    })))
}


// ============================================================================
// 仪表盘 API (Dashboard)
// ============================================================================

async fn get_dashboard_stats(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let total_users: i64 = client
        .query_one("SELECT COUNT(*) FROM users", &[])
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .get(0);

    let online_clients: i64 = client
        .query_one("SELECT COUNT(*) FROM registered_clients WHERE online = true", &[])
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .get(0);

    let dlp_blocked_today: i64 = client
        .query_one(
            "SELECT COUNT(*) FROM audit_logs WHERE action = 'dlp_block' AND created_at >= CURRENT_DATE",
            &[],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .get(0);

    let sensitive_ops_today: i64 = client
        .query_one(
            "SELECT COUNT(*) FROM audit_logs WHERE action LIKE 'sensitive_%' AND created_at >= CURRENT_DATE",
            &[],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .get(0);

    // 未处理告警数（alert_events 表可能尚未创建，查询失败时返回 0）
    let unhandled_alerts: i64 = client
        .query_one(
            "SELECT COUNT(*) FROM alert_events WHERE status IN ('pending', 'in_progress')",
            &[],
        )
        .await
        .map(|r| r.get(0))
        .unwrap_or(0);

    Ok(Json(json!({
        "total_users": total_users,
        "online_clients": online_clients,
        "dlp_blocked_today": dlp_blocked_today,
        "sensitive_ops_today": sensitive_ops_today,
        "unhandled_alerts": unhandled_alerts,
    })))
}

async fn get_dashboard_activity(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let rows = client
        .query(
            "SELECT a.id, u.username, a.action, a.details, a.created_at
             FROM audit_logs a
             LEFT JOIN users u ON a.user_id = u.id
             ORDER BY a.created_at DESC
             LIMIT 20",
            &[],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let logs: Vec<_> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<_, Uuid>(0),
                "username": r.get::<_, Option<String>>(1),
                "action": r.get::<_, String>(2),
                "details": r.get::<_, String>(3),
                "created_at": r.get::<_, chrono::DateTime<chrono::Utc>>(4),
            })
        })
        .collect();

    Ok(Json(json!({ "logs": logs })))
}

async fn get_dashboard_trends(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    // 近 7 天用户活跃度（按审计日志中不同 user_id 计数）
    let user_activity_rows = client
        .query(
            "SELECT d::date as date, COUNT(DISTINCT a.user_id) as count
             FROM generate_series(CURRENT_DATE - INTERVAL '6 days', CURRENT_DATE, '1 day') d
             LEFT JOIN audit_logs a ON DATE(a.created_at) = d::date
             GROUP BY d::date
             ORDER BY d::date",
            &[],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let user_activity: Vec<_> = user_activity_rows
        .iter()
        .map(|r| {
            json!({
                "date": r.get::<_, chrono::NaiveDate>(0).to_string(),
                "count": r.get::<_, i64>(1),
            })
        })
        .collect();

    // 近 7 天 DLP 拦截次数
    let dlp_blocks_rows = client
        .query(
            "SELECT d::date as date, COUNT(a.id) as count
             FROM generate_series(CURRENT_DATE - INTERVAL '6 days', CURRENT_DATE, '1 day') d
             LEFT JOIN audit_logs a ON DATE(a.created_at) = d::date AND a.action = 'dlp_block'
             GROUP BY d::date
             ORDER BY d::date",
            &[],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let dlp_blocks: Vec<_> = dlp_blocks_rows
        .iter()
        .map(|r| {
            json!({
                "date": r.get::<_, chrono::NaiveDate>(0).to_string(),
                "count": r.get::<_, i64>(1),
            })
        })
        .collect();

    Ok(Json(json!({
        "user_activity": user_activity,
        "dlp_blocks": dlp_blocks,
    })))
}


// ============================================================================
// 技能/插件启用禁用 (Skill/Plugin Enable/Disable)
// ============================================================================

async fn toggle_skill(
    state: &AppState,
    skill_id: Uuid,
    enabled: bool,
) -> Result<Json<serde_json::Value>> {
    let db = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    // 检查技能是否存在（先查本地数据库）
    let row = db
        .query_opt("SELECT id, name FROM skills WHERE id = $1", &[&skill_id])
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    if row.is_none() {
        return Err(Error::SkillNotFound);
    }
    let skill_name: String = row.unwrap().get(1);

    // 尝试代理到 Gateway
    let action = if enabled { "enable" } else { "disable" };
    let gateway_url = format!("{}/api/skills/{}/{}", state.gateway_url, skill_id, action);
    let gateway_synced = match state.http_client.post(&gateway_url).send().await {
        Ok(resp) if resp.status().is_success() => true,
        _ => false,
    };

    // 更新本地数据库
    let now = chrono::Utc::now();
    db.execute(
        "UPDATE skills SET enabled = $1, updated_at = $2 WHERE id = $3",
        &[&enabled, &now, &skill_id],
    )
    .await
    .map_err(|e| Error::Database(e.to_string()))?;

    // 审计日志
    let audit_action = if enabled { "enable_skill" } else { "disable_skill" };
    let system_user = Uuid::nil();
    write_audit_log(&db, system_user, audit_action, &format!("{}: {}", audit_action, skill_name)).await;

    Ok(Json(json!({
        "id": skill_id,
        "name": skill_name,
        "enabled": enabled,
        "gateway_synced": gateway_synced,
    })))
}

async fn enable_skill(
    State(state): State<AppState>,
    Path(skill_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    toggle_skill(&state, skill_id, true).await
}

async fn disable_skill(
    State(state): State<AppState>,
    Path(skill_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    toggle_skill(&state, skill_id, false).await
}

async fn toggle_plugin(
    state: &AppState,
    plugin_id: Uuid,
    enabled: bool,
) -> Result<Json<serde_json::Value>> {
    let db = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let row = db
        .query_opt("SELECT id, name FROM plugins WHERE id = $1", &[&plugin_id])
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    if row.is_none() {
        return Err(Error::PluginNotFound);
    }
    let plugin_name: String = row.unwrap().get(1);

    let action = if enabled { "enable" } else { "disable" };
    let gateway_url = format!("{}/api/extensions/{}/{}", state.gateway_url, plugin_id, action);
    let gateway_synced = match state.http_client.post(&gateway_url).send().await {
        Ok(resp) if resp.status().is_success() => true,
        _ => false,
    };

    let now = chrono::Utc::now();
    db.execute(
        "UPDATE plugins SET enabled = $1, updated_at = $2 WHERE id = $3",
        &[&enabled, &now, &plugin_id],
    )
    .await
    .map_err(|e| Error::Database(e.to_string()))?;

    let audit_action = if enabled { "enable_plugin" } else { "disable_plugin" };
    let system_user = Uuid::nil();
    write_audit_log(&db, system_user, audit_action, &format!("{}: {}", audit_action, plugin_name)).await;

    Ok(Json(json!({
        "id": plugin_id,
        "name": plugin_name,
        "enabled": enabled,
        "gateway_synced": gateway_synced,
    })))
}

async fn enable_plugin(
    State(state): State<AppState>,
    Path(plugin_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    toggle_plugin(&state, plugin_id, true).await
}

async fn disable_plugin(
    State(state): State<AppState>,
    Path(plugin_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    toggle_plugin(&state, plugin_id, false).await
}


// ============================================================================
// 客户端配置更新 (PUT /api/client-config)
// ============================================================================

async fn update_client_config(
    State(state): State<AppState>,
    Json(payload): Json<UpdateClientConfigRequest>,
) -> Result<Json<serde_json::Value>> {
    // 验证 max_cost_per_day_cents
    if let Some(cost) = payload.max_cost_per_day_cents {
        if cost < 0 {
            return Err(Error::Validation("max_cost_per_day_cents 必须 >= 0".to_string()));
        }
    }

    // 验证 llm_base_url
    if let Some(ref url) = payload.llm_base_url {
        if !url.is_empty() && !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(Error::Validation("llm_base_url 格式不正确，需以 http:// 或 https:// 开头".to_string()));
        }
    }

    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let now = chrono::Utc::now();

    // 检查全局配置是否存在
    let existing = client
        .query_opt("SELECT id, config_version FROM client_configs WHERE client_id IS NULL", &[])
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let config_version: i64;

    if let Some(row) = existing {
        let current_version: i64 = row.get(1);
        config_version = current_version + 1;
        let config_id: Uuid = row.get(0);

        // 动态构建 UPDATE
        let mut set_clauses = vec![
            "config_version = $1".to_string(),
            "updated_at = $2".to_string(),
        ];
        let mut params: Vec<Box<dyn tokio_postgres::types::ToSql + Sync + Send>> = vec![
            Box::new(config_version),
            Box::new(now),
        ];
        let mut param_idx = 3u32;
        let mut changed_fields: Vec<String> = vec![];

        if let Some(ref v) = payload.llm_backend {
            set_clauses.push(format!("llm_backend = ${}", param_idx));
            params.push(Box::new(v.clone()));
            param_idx += 1;
            changed_fields.push("llm_backend".to_string());
        }
        if let Some(ref v) = payload.llm_api_key {
            set_clauses.push(format!("llm_api_key = ${}", param_idx));
            params.push(Box::new(v.clone()));
            param_idx += 1;
            changed_fields.push("llm_api_key".to_string());
        }
        if let Some(ref v) = payload.llm_model {
            set_clauses.push(format!("llm_model = ${}", param_idx));
            params.push(Box::new(v.clone()));
            param_idx += 1;
            changed_fields.push("llm_model".to_string());
        }
        if let Some(ref v) = payload.llm_base_url {
            set_clauses.push(format!("llm_base_url = ${}", param_idx));
            params.push(Box::new(v.clone()));
            param_idx += 1;
            changed_fields.push("llm_base_url".to_string());
        }
        if let Some(v) = payload.safety_enabled {
            set_clauses.push(format!("safety_enabled = ${}", param_idx));
            params.push(Box::new(v));
            param_idx += 1;
            changed_fields.push("safety_enabled".to_string());
        }
        if let Some(v) = payload.skills_enabled {
            set_clauses.push(format!("skills_enabled = ${}", param_idx));
            params.push(Box::new(v));
            param_idx += 1;
            changed_fields.push("skills_enabled".to_string());
        }
        if let Some(v) = payload.extensions_enabled {
            set_clauses.push(format!("extensions_enabled = ${}", param_idx));
            params.push(Box::new(v));
            param_idx += 1;
            changed_fields.push("extensions_enabled".to_string());
        }
        if let Some(v) = payload.max_cost_per_day_cents {
            set_clauses.push(format!("max_cost_per_day_cents = ${}", param_idx));
            params.push(Box::new(v));
            param_idx += 1;
            changed_fields.push("max_cost_per_day_cents".to_string());
        }

        params.push(Box::new(config_id));

        let sql = format!(
            "UPDATE client_configs SET {} WHERE id = ${}",
            set_clauses.join(", "),
            param_idx
        );

        let param_refs: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
            params.iter().map(|p| p.as_ref() as &(dyn tokio_postgres::types::ToSql + Sync)).collect();

        client.execute(&sql, &param_refs).await
            .map_err(|e| Error::Database(e.to_string()))?;

        // 审计日志（不记录 api_key 值）
        let system_user = Uuid::nil();
        write_audit_log(
            &client,
            system_user,
            "update_client_config",
            &format!("更新客户端配置 v{}，变更字段: {}", config_version, changed_fields.join(", ")),
        ).await;
    } else {
        // 不存在全局配置，创建一条
        config_version = 1;
        let id = Uuid::new_v4();
        client.execute(
            "INSERT INTO client_configs (id, client_id, llm_backend, llm_api_key, llm_model, llm_base_url, safety_enabled, skills_enabled, extensions_enabled, max_cost_per_day_cents, config_version, updated_at, created_at)
             VALUES ($1, NULL, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
            &[
                &id,
                &payload.llm_backend, &payload.llm_api_key, &payload.llm_model, &payload.llm_base_url,
                &payload.safety_enabled, &payload.skills_enabled, &payload.extensions_enabled,
                &payload.max_cost_per_day_cents, &config_version, &now, &now,
            ],
        ).await.map_err(|e| Error::Database(e.to_string()))?;

        let system_user = Uuid::nil();
        write_audit_log(&client, system_user, "update_client_config", "创建全局客户端配置 v1").await;
    }

    Ok(Json(json!({
        "message": "配置保存成功",
        "config_version": config_version,
        "updated_at": now,
    })))
}


// ============================================================================
// 审计日志导出 (GET /api/audit-logs/export)
// ============================================================================

async fn export_audit_logs(
    State(state): State<AppState>,
    Query(params): Query<AuditLogExportQuery>,
) -> Result<axum::response::Response> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let mut conditions: Vec<String> = vec![];
    let mut query_params: Vec<Box<dyn tokio_postgres::types::ToSql + Sync + Send>> = vec![];
    let mut idx = 1u32;

    if let Some(ref start_time) = params.start_time {
        let dt = chrono::DateTime::parse_from_rfc3339(start_time)
            .or_else(|_| chrono::NaiveDateTime::parse_from_str(start_time, "%Y-%m-%dT%H:%M:%S")
                .map(|ndt| ndt.and_utc().fixed_offset()))
            .map_err(|_| Error::Validation("start_time 格式不正确，需为 ISO 8601 格式".to_string()))?;
        conditions.push(format!("a.created_at >= ${}", idx));
        query_params.push(Box::new(dt.with_timezone(&chrono::Utc)));
        idx += 1;
    }

    if let Some(ref end_time) = params.end_time {
        let dt = chrono::DateTime::parse_from_rfc3339(end_time)
            .or_else(|_| chrono::NaiveDateTime::parse_from_str(end_time, "%Y-%m-%dT%H:%M:%S")
                .map(|ndt| ndt.and_utc().fixed_offset()))
            .map_err(|_| Error::Validation("end_time 格式不正确，需为 ISO 8601 格式".to_string()))?;
        conditions.push(format!("a.created_at <= ${}", idx));
        query_params.push(Box::new(dt.with_timezone(&chrono::Utc)));
        idx += 1;
    }

    if let Some(ref action) = params.action {
        conditions.push(format!("a.action = ${}", idx));
        query_params.push(Box::new(action.clone()));
        idx += 1;
    }

    if let Some(ref username) = params.username {
        let pattern = format!("%{}%", username);
        conditions.push(format!("u.username ILIKE ${}", idx));
        query_params.push(Box::new(pattern));
        let _ = idx;
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let sql = format!(
        "SELECT a.id, u.username, a.action, a.details, a.created_at
         FROM audit_logs a
         LEFT JOIN users u ON a.user_id = u.id
         {}
         ORDER BY a.created_at DESC",
        where_clause
    );

    let param_refs: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
        query_params.iter().map(|p| p.as_ref() as &(dyn tokio_postgres::types::ToSql + Sync)).collect();

    let rows = client.query(&sql, &param_refs).await
        .map_err(|e| Error::Database(e.to_string()))?;

    // 构建 CSV
    let bom = "\u{FEFF}";
    let header = "时间,操作人,操作类型,详情,日志ID\n";
    let mut csv = format!("{}{}", bom, header);

    for row in &rows {
        let id: Uuid = row.get(0);
        let username: Option<String> = row.get(1);
        let action: String = row.get(2);
        let details: String = row.get(3);
        let created_at: chrono::DateTime<chrono::Utc> = row.get(4);

        csv.push_str(&format!(
            "{},{},{},{},{}\n",
            escape_csv_field(&created_at.to_rfc3339()),
            escape_csv_field(username.as_deref().unwrap_or("系统")),
            escape_csv_field(&action),
            escape_csv_field(&details),
            escape_csv_field(&id.to_string()),
        ));
    }

    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let filename = format!("audit-logs-{}.csv", today);

    Ok(axum::response::Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/csv; charset=utf-8")
        .header("Content-Disposition", format!("attachment; filename=\"{}\"", filename))
        .body(axum::body::Body::from(csv))
        .unwrap())
}


// ============================================================================
// 客户端强制下线与策略推送 (Client Operations)
// ============================================================================

/// POST /api/clients/{id}/disconnect — 强制客户端下线
///
/// 将客户端标记为离线状态，并记录审计日志。
async fn disconnect_client(
    State(state): State<AppState>,
    Path(client_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    // 检查客户端是否存在
    let existing = client
        .query_opt(
            "SELECT id, username, online FROM registered_clients WHERE id = $1",
            &[&client_id],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .ok_or(Error::ClientNotFound)?;

    let username: Option<String> = existing.try_get::<_, Option<String>>(1).unwrap_or(None);
    let was_online: bool = existing.get(2);

    if !was_online {
        return Ok(Json(json!({
            "message": "客户端已处于离线状态",
            "client_id": client_id,
            "was_online": false,
        })));
    }

    let now = chrono::Utc::now();
    client
        .execute(
            "UPDATE registered_clients SET online = false, updated_at = $1 WHERE id = $2",
            &[&now, &client_id],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    // 审计日志
    let system_user = Uuid::nil();
    let details = format!(
        "强制下线客户端: {} (用户: {})",
        client_id,
        username.as_deref().unwrap_or("未知")
    );
    write_audit_log(&client, system_user, "disconnect_client", &details).await;

    Ok(Json(json!({
        "message": "客户端已强制下线",
        "client_id": client_id,
        "was_online": true,
        "disconnected_at": now,
    })))
}

/// POST /api/clients/{id}/push-policy — 向指定客户端推送策略
///
/// 更新客户端的 policy_version 字段，标记需要同步最新策略。
async fn push_policy_to_client(
    State(state): State<AppState>,
    Path(client_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    // 检查客户端是否存在
    let existing = client
        .query_opt(
            "SELECT id, username FROM registered_clients WHERE id = $1",
            &[&client_id],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .ok_or(Error::ClientNotFound)?;

    let username: Option<String> = existing.try_get::<_, Option<String>>(1).unwrap_or(None);

    // 生成新的策略版本号（时间戳格式）
    let now = chrono::Utc::now();
    let policy_version = now.format("%Y%m%d%H%M%S").to_string();

    client
        .execute(
            "UPDATE registered_clients SET policy_version = $1, updated_at = $2 WHERE id = $3",
            &[&policy_version, &now, &client_id],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    // 审计日志
    let system_user = Uuid::nil();
    let details = format!(
        "推送策略到客户端: {} (用户: {}, 版本: {})",
        client_id,
        username.as_deref().unwrap_or("未知"),
        policy_version
    );
    write_audit_log(&client, system_user, "push_policy", &details).await;

    Ok(Json(json!({
        "message": "策略推送成功",
        "client_id": client_id,
        "policy_version": policy_version,
        "pushed_at": now,
    })))
}

/// POST /api/clients/push-policy-all — 向所有在线客户端推送策略
async fn push_policy_all(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let now = chrono::Utc::now();
    let policy_version = now.format("%Y%m%d%H%M%S").to_string();

    let result = client
        .execute(
            "UPDATE registered_clients SET policy_version = $1, updated_at = $2 WHERE online = true",
            &[&policy_version, &now],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    // 审计日志
    let system_user = Uuid::nil();
    let details = format!(
        "批量推送策略到所有在线客户端: {} 台 (版本: {})",
        result, policy_version
    );
    write_audit_log(&client, system_user, "push_policy_all", &details).await;

    Ok(Json(json!({
        "message": format!("策略已推送到 {} 台在线客户端", result),
        "updated_count": result,
        "policy_version": policy_version,
        "pushed_at": now,
    })))
}


// ============================================================================
// 部门管理 CRUD (Department Management)
// ============================================================================

/// GET /api/departments — 获取部门列表
///
/// LEFT JOIN 计算每个部门的成员数量。
/// 当 token_quota_enabled = false 时，token_quota_per_day 返回 null。
async fn get_departments(
    State(state): State<AppState>,
    Query(params): Query<models::DepartmentQuery>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let (sql, query_params) = build_departments_query(&params);
    let param_refs: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
        query_params.iter().map(|p| p.as_ref() as &(dyn tokio_postgres::types::ToSql + Sync)).collect();

    let rows = client.query(&sql, &param_refs).await
        .map_err(|e| Error::Database(e.to_string()))?;

    let departments: Vec<_> = rows.iter().map(|r| {
        let quota_enabled: bool = r.get(3);
        json!({
            "id": r.get::<_, Uuid>(0),
            "name": r.get::<_, String>(1),
            "description": r.get::<_, Option<String>>(2),
            "token_quota_enabled": quota_enabled,
            "token_quota_per_day": if quota_enabled { r.get::<_, Option<i32>>(4) } else { None },
            "parent_id": r.get::<_, Option<Uuid>>(5),
            "created_at": r.get::<_, chrono::DateTime<chrono::Utc>>(6),
            "updated_at": r.get::<_, chrono::DateTime<chrono::Utc>>(7),
            "member_count": r.get::<_, i64>(8),
        })
    }).collect();

    Ok(Json(json!({ "departments": departments })))
}

/// 构建部门列表查询 SQL（支持搜索 + 筛选）
fn build_departments_query(
    params: &models::DepartmentQuery,
) -> (String, Vec<Box<dyn tokio_postgres::types::ToSql + Sync + Send>>) {
    let mut sql = String::from(
        "SELECT d.id, d.name, d.description, d.token_quota_enabled, d.token_quota_per_day,
                d.parent_id, d.created_at, d.updated_at,
                COUNT(u.id) as member_count
         FROM departments d
         LEFT JOIN users u ON u.department_id = d.id"
    );
    let mut conditions: Vec<String> = Vec::new();
    let mut query_params: Vec<Box<dyn tokio_postgres::types::ToSql + Sync + Send>> = Vec::new();
    let mut idx = 1u32;

    if let Some(ref search) = params.search {
        let trimmed = search.trim();
        if !trimmed.is_empty() {
            conditions.push(format!("d.name ILIKE ${}", idx));
            query_params.push(Box::new(format!("%{}%", trimmed)));
            idx += 1;
        }
    }

    if let Some(ref status) = params.quota_status {
        match status.as_str() {
            "enabled" => conditions.push("d.token_quota_enabled = true".to_string()),
            "disabled" => conditions.push("d.token_quota_enabled = false".to_string()),
            _ => {} // 忽略无效值
        }
    }

    if !conditions.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&conditions.join(" AND "));
    }

    sql.push_str(
        " GROUP BY d.id, d.name, d.description, d.token_quota_enabled, d.token_quota_per_day,
                   d.parent_id, d.created_at, d.updated_at
          ORDER BY d.name ASC"
    );

    let _ = idx; // suppress unused warning
    (sql, query_params)
}

/// POST /api/departments — 创建部门
///
/// 验证：
/// - name 长度 2-100 字符
/// - name 唯一性（409 Conflict）
/// - token_quota 一致性：启用限额时必须提供 token_quota_per_day
/// - parent_id 有效性（如果提供）
async fn create_department(
    State(state): State<AppState>,
    Json(payload): Json<CreateDepartmentRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>)> {
    // 验证名称长度
    let name = payload.name.trim().to_string();
    if name.len() < 2 || name.len() > 100 {
        return Err(Error::Validation("部门名称长度必须在 2-100 字符之间".to_string()));
    }

    // 验证 token_quota 一致性
    let quota_enabled = payload.token_quota_enabled.unwrap_or(false);
    if quota_enabled && payload.token_quota_per_day.is_none() {
        return Err(Error::Validation(
            "启用 Token 限额时必须提供 token_quota_per_day".to_string(),
        ));
    }
    if let Some(quota) = payload.token_quota_per_day {
        if quota < 0 {
            return Err(Error::Validation("token_quota_per_day 不能为负数".to_string()));
        }
    }

    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    // 检查名称唯一性
    let existing = client
        .query_opt("SELECT id FROM departments WHERE name = $1", &[&name])
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    if existing.is_some() {
        return Err(Error::Conflict(format!("部门名称 '{}' 已存在", name)));
    }

    // 验证 parent_id 有效性
    if let Some(ref parent_id) = payload.parent_id {
        let parent_exists = client
            .query_opt("SELECT id FROM departments WHERE id = $1", &[parent_id])
            .await
            .map_err(|e| Error::Database(e.to_string()))?;
        if parent_exists.is_none() {
            return Err(Error::Validation("指定的父部门不存在".to_string()));
        }
    }

    let dept_id = Uuid::new_v4();
    let now = chrono::Utc::now();
    let quota_per_day = if quota_enabled {
        payload.token_quota_per_day
    } else {
        None
    };

    client
        .execute(
            "INSERT INTO departments (id, name, description, parent_id, token_quota_enabled, token_quota_per_day, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
            &[&dept_id, &name, &payload.description, &payload.parent_id, &quota_enabled, &quota_per_day, &now, &now],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    // 审计日志
    let system_user = Uuid::nil();
    write_audit_log(
        &client,
        system_user,
        "create_department",
        &format!("创建部门: {} (限额: {})", name, if quota_enabled { "启用" } else { "关闭" }),
    )
    .await;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": dept_id,
            "name": name,
            "description": payload.description,
            "parent_id": payload.parent_id,
            "token_quota_enabled": quota_enabled,
            "token_quota_per_day": quota_per_day,
            "created_at": now,
            "updated_at": now,
            "member_count": 0,
        })),
    ))
}

/// PUT /api/departments/{id} — 更新部门
async fn update_department(
    State(state): State<AppState>,
    Path(dept_id): Path<Uuid>,
    Json(payload): Json<UpdateDepartmentRequest>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    // 检查部门是否存在
    let existing = client
        .query_opt("SELECT id FROM departments WHERE id = $1", &[&dept_id])
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    if existing.is_none() {
        return Err(Error::DepartmentNotFound);
    }

    // 验证名称
    if let Some(ref name) = payload.name {
        let trimmed = name.trim();
        if trimmed.len() < 2 || trimmed.len() > 100 {
            return Err(Error::Validation("部门名称长度必须在 2-100 字符之间".to_string()));
        }
        // 检查名称唯一性（排除自身）
        let dup = client
            .query_opt(
                "SELECT id FROM departments WHERE name = $1 AND id != $2",
                &[&trimmed.to_string(), &dept_id],
            )
            .await
            .map_err(|e| Error::Database(e.to_string()))?;
        if dup.is_some() {
            return Err(Error::Conflict(format!("部门名称 '{}' 已存在", trimmed)));
        }
    }

    // 验证 parent_id（不能设为自身，不能形成循环）
    if let Some(ref parent_opt) = payload.parent_id {
        if let Some(ref parent_id) = parent_opt {
            if *parent_id == dept_id {
                return Err(Error::Validation("部门不能设为自身的子部门".to_string()));
            }
            let parent_exists = client
                .query_opt("SELECT id FROM departments WHERE id = $1", &[parent_id])
                .await
                .map_err(|e| Error::Database(e.to_string()))?;
            if parent_exists.is_none() {
                return Err(Error::Validation("指定的父部门不存在".to_string()));
            }
        }
    }

    let now = chrono::Utc::now();
    let mut set_clauses = vec!["updated_at = $1".to_string()];
    let mut params: Vec<Box<dyn tokio_postgres::types::ToSql + Sync + Send>> = vec![Box::new(now)];
    let mut param_idx = 2u32;
    let mut changed_fields: Vec<String> = vec![];

    if let Some(ref name) = payload.name {
        set_clauses.push(format!("name = ${}", param_idx));
        params.push(Box::new(name.trim().to_string()));
        param_idx += 1;
        changed_fields.push("name".to_string());
    }

    if let Some(ref description) = payload.description {
        set_clauses.push(format!("description = ${}", param_idx));
        params.push(Box::new(description.clone()));
        param_idx += 1;
        changed_fields.push("description".to_string());
    }

    if let Some(ref parent_id) = payload.parent_id {
        set_clauses.push(format!("parent_id = ${}", param_idx));
        params.push(Box::new(*parent_id));
        param_idx += 1;
        changed_fields.push("parent_id".to_string());
    }

    if let Some(quota_enabled) = payload.token_quota_enabled {
        set_clauses.push(format!("token_quota_enabled = ${}", param_idx));
        params.push(Box::new(quota_enabled));
        param_idx += 1;
        changed_fields.push("token_quota_enabled".to_string());
    }

    if let Some(ref quota_per_day) = payload.token_quota_per_day {
        set_clauses.push(format!("token_quota_per_day = ${}", param_idx));
        params.push(Box::new(*quota_per_day));
        param_idx += 1;
        changed_fields.push("token_quota_per_day".to_string());
    }

    if changed_fields.is_empty() {
        return Err(Error::Validation("至少需要提供一个要更新的字段".to_string()));
    }

    params.push(Box::new(dept_id));

    let sql = format!(
        "UPDATE departments SET {} WHERE id = ${}
         RETURNING id, name, description, parent_id, token_quota_enabled, token_quota_per_day, created_at, updated_at",
        set_clauses.join(", "),
        param_idx
    );

    let param_refs: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
        params.iter().map(|p| p.as_ref() as &(dyn tokio_postgres::types::ToSql + Sync)).collect();

    let row = client
        .query_one(&sql, &param_refs)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    // 审计日志
    let system_user = Uuid::nil();
    write_audit_log(
        &client,
        system_user,
        "update_department",
        &format!("更新部门 {}，修改字段: {}", row.get::<_, String>(1), changed_fields.join(", ")),
    )
    .await;

    // 查询成员数
    let count_row = client
        .query_one(
            "SELECT COUNT(*) FROM users WHERE department_id = $1",
            &[&dept_id],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?;
    let member_count: i64 = count_row.get(0);

    let quota_enabled: bool = row.get(4);
    let quota_per_day: Option<i32> = if quota_enabled { row.get(5) } else { None };

    Ok(Json(json!({
        "id": row.get::<_, Uuid>(0),
        "name": row.get::<_, String>(1),
        "description": row.get::<_, Option<String>>(2),
        "parent_id": row.get::<_, Option<Uuid>>(3),
        "token_quota_enabled": quota_enabled,
        "token_quota_per_day": quota_per_day,
        "created_at": row.get::<_, chrono::DateTime<chrono::Utc>>(6),
        "updated_at": row.get::<_, chrono::DateTime<chrono::Utc>>(7),
        "member_count": member_count,
    })))
}

/// DELETE /api/departments/{id} — 删除部门
///
/// 验证：有成员或子部门时禁止删除。
/// 清理：删除关联的模型白名单（由 ON DELETE CASCADE 处理）。
async fn delete_department(
    State(state): State<AppState>,
    Path(dept_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    // 检查部门是否存在
    let existing = client
        .query_opt(
            "SELECT id, name FROM departments WHERE id = $1",
            &[&dept_id],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .ok_or(Error::DepartmentNotFound)?;

    let dept_name: String = existing.get(1);

    // 检查是否有用户属于该部门
    let user_count: i64 = client
        .query_one(
            "SELECT COUNT(*) FROM users WHERE department_id = $1",
            &[&dept_id],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .get(0);

    if user_count > 0 {
        return Err(Error::DepartmentHasUsers);
    }

    // 检查是否有子部门
    let child_count: i64 = client
        .query_one(
            "SELECT COUNT(*) FROM departments WHERE parent_id = $1",
            &[&dept_id],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .get(0);

    if child_count > 0 {
        return Err(Error::Validation(
            format!("部门 '{}' 下有 {} 个子部门，请先删除或移动子部门", dept_name, child_count),
        ));
    }

    // 删除部门（模型白名单由 ON DELETE CASCADE 自动清理）
    client
        .execute("DELETE FROM departments WHERE id = $1", &[&dept_id])
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    // 审计日志
    let system_user = Uuid::nil();
    write_audit_log(
        &client,
        system_user,
        "delete_department",
        &format!("删除部门: {}", dept_name),
    )
    .await;

    Ok(Json(json!({ "message": "删除成功" })))
}

// ============================================================================
// 模型配置 API
// ============================================================================

/// GET /api/model-configs — 获取所有模型配置（管理端）
#[instrument(skip(state))]
async fn get_model_configs(
    State(state): State<AppState>,
) -> Result<Json<Vec<serde_json::Value>>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let rows = client
        .query(
            "SELECT id, model_id, display_name, description, provider, \
             api_base_url, api_key, enabled, is_default, sort_order, \
             capabilities, extra_config, created_at, updated_at \
             FROM model_configs ORDER BY sort_order, display_name",
            &[],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let configs: Vec<serde_json::Value> = rows
        .iter()
        .map(|row| {
            let api_key: Option<String> = row.get(6);
            json!({
                "id": row.get::<_, Uuid>(0),
                "model_id": row.get::<_, String>(1),
                "display_name": row.get::<_, String>(2),
                "description": row.get::<_, Option<String>>(3),
                "provider": row.get::<_, String>(4),
                "api_base_url": row.get::<_, Option<String>>(5),
                "api_key": api_key.as_deref().map(mask_api_key),
                "enabled": row.get::<_, bool>(7),
                "is_default": row.get::<_, bool>(8),
                "sort_order": row.get::<_, i32>(9),
                "capabilities": row.get::<_, serde_json::Value>(10),
                "extra_config": row.get::<_, serde_json::Value>(11),
                "created_at": row.get::<_, DateTime<Utc>>(12),
                "updated_at": row.get::<_, DateTime<Utc>>(13),
            })
        })
        .collect();

    Ok(Json(configs))
}

/// POST /api/model-configs — 创建模型配置
#[instrument(skip(state, body))]
async fn create_model_config(
    State(state): State<AppState>,
    Json(body): Json<models::CreateModelConfigRequest>,
) -> Result<Json<serde_json::Value>> {
    // 验证必填字段
    if body.model_id.trim().is_empty() {
        return Err(Error::Validation("model_id 不能为空".into()));
    }
    if body.display_name.trim().is_empty() {
        return Err(Error::Validation("display_name 不能为空".into()));
    }
    if body.api_key.as_deref().unwrap_or("").trim().is_empty() {
        return Err(Error::Validation("api_key 不能为空".into()));
    }

    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    // 检查 model_id 唯一性
    let existing = client
        .query_opt(
            "SELECT id FROM model_configs WHERE model_id = $1",
            &[&body.model_id],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    if existing.is_some() {
        return Err(Error::Validation(format!(
            "model_id '{}' 已存在",
            body.model_id
        )));
    }

    let id = Uuid::new_v4();
    let capabilities = body.capabilities.unwrap_or(json!([]));
    let extra_config = body.extra_config.unwrap_or(json!({}));
    let normalized_url = body.api_base_url.as_deref().map(normalize_api_base_url);

    client
        .execute(
            "INSERT INTO model_configs \
             (id, model_id, display_name, description, provider, api_base_url, \
              api_key, sort_order, capabilities, extra_config) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
            &[
                &id,
                &body.model_id,
                &body.display_name,
                &body.description,
                &body.provider,
                &normalized_url,
                &body.api_key,
                &body.sort_order,
                &capabilities,
                &extra_config,
            ],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    // 审计日志
    let system_user = Uuid::nil();
    write_audit_log(
        &client,
        system_user,
        "create_model_config",
        &format!("创建模型配置: {} ({})", body.display_name, body.model_id),
    )
    .await;

    Ok(Json(json!({
        "id": id,
        "model_id": body.model_id,
        "display_name": body.display_name,
        "message": "创建成功"
    })))
}

/// PUT /api/model-configs/{id} — 更新模型配置
#[instrument(skip_all)]
async fn update_model_config(
    State(state): State<AppState>,
    Path(config_id): Path<Uuid>,
    Json(body): Json<models::UpdateModelConfigRequest>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    // 检查是否存在
    let existing = client
        .query_opt(
            "SELECT display_name FROM model_configs WHERE id = $1",
            &[&config_id],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .ok_or(Error::Validation("模型配置不存在".into()))?;

    let old_name: String = existing.get(0);

    // 如果设为默认，先清除其他默认
    if body.is_default == Some(true) {
        client
            .execute(
                "UPDATE model_configs SET is_default = false WHERE is_default = true AND id != $1",
                &[&config_id],
            )
            .await
            .map_err(|e| Error::Database(e.to_string()))?;
    }

    // 用 COALESCE 保留未传字段的原值，避免动态 SQL + Box<dyn ToSql> 的 Send 问题
    let normalized_url = body.api_base_url.as_deref().map(normalize_api_base_url);
    client
        .execute(
            "UPDATE model_configs SET \
                display_name  = COALESCE($2, display_name), \
                description   = COALESCE($3, description), \
                provider      = COALESCE($4, provider), \
                api_base_url  = COALESCE($5, api_base_url), \
                api_key       = COALESCE($6, api_key), \
                enabled       = COALESCE($7, enabled), \
                is_default    = COALESCE($8, is_default), \
                sort_order    = COALESCE($9, sort_order), \
                capabilities  = COALESCE($10, capabilities), \
                extra_config  = COALESCE($11, extra_config), \
                updated_at    = NOW() \
             WHERE id = $1",
            &[
                &config_id,
                &body.display_name,
                &body.description,
                &body.provider,
                &normalized_url,
                &body.api_key,
                &body.enabled,
                &body.is_default,
                &body.sort_order,
                &body.capabilities,
                &body.extra_config,
            ],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    // 审计日志
    let system_user = Uuid::nil();
    write_audit_log(
        &client,
        system_user,
        "update_model_config",
        &format!("更新模型配置: {}", old_name),
    )
    .await;

    Ok(Json(json!({ "message": "更新成功" })))
}

/// DELETE /api/model-configs/{id} — 删除模型配置
#[instrument(skip(state))]
async fn delete_model_config(
    State(state): State<AppState>,
    Path(config_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let existing = client
        .query_opt(
            "SELECT display_name, is_default FROM model_configs WHERE id = $1",
            &[&config_id],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .ok_or(Error::Validation("模型配置不存在".into()))?;

    let name: String = existing.get(0);
    let is_default: bool = existing.get(1);

    if is_default {
        return Err(Error::Validation("不能删除默认模型".into()));
    }

    client
        .execute("DELETE FROM model_configs WHERE id = $1", &[&config_id])
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    // 审计日志
    let system_user = Uuid::nil();
    write_audit_log(
        &client,
        system_user,
        "delete_model_config",
        &format!("删除模型配置: {}", name),
    )
    .await;

    Ok(Json(json!({ "message": "删除成功" })))
}

/// POST /api/model-configs/test-connection — 测试模型 API 连接
#[instrument(skip(_state, body))]
async fn test_model_connection(
    State(_state): State<AppState>,
    Json(body): Json<models::TestConnectionRequest>,
) -> Result<Json<serde_json::Value>> {
    let base_url = body.api_base_url.as_deref().unwrap_or_default();
    if base_url.is_empty() {
        return Err(Error::Validation("api_base_url 不能为空".to_string()));
    }

    let api_key = body.api_key.as_deref().unwrap_or_default();
    if api_key.is_empty() {
        return Err(Error::Validation("api_key 不能为空".to_string()));
    }

    // 测试连接用独立 client，超时比全局 http_client 更宽裕
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| Error::Internal(format!("创建 HTTP 客户端失败: {}", e)))?;

    // 用真实 model_id 测试（部分 provider 如智谱对不存在的模型返回 403）
    let model_id = body.model_id.as_deref().unwrap_or("test");

    let test_body = serde_json::json!({
        "model": model_id,
        "messages": [{"role": "user", "content": "hi"}],
        "max_tokens": 1,
    });

    let response = client
        .post(&url)
        .json(&test_body)
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .map_err(|e| Error::Internal(format!("连接失败: {}", e)))?;

    let status = response.status().as_u16();
    match status {
        // 2xx: 完全成功（LLM 正常返回）
        200..=299 => Ok(Json(serde_json::json!({
            "success": true,
            "message": "连接成功",
            "status": status,
        }))),
        // 401: 认证失败，API Key 无效
        401 => Err(Error::Validation(format!(
            "认证失败 ({}): API Key 无效",
            status
        ))),
        // 400/403/404/422: 认证可能通过但模型/请求有误，视为连接成功
        // 注意：部分 provider（如智谱）对不存在的模型返回 403 而非 404
        400 | 403 | 404 | 422 => Ok(Json(serde_json::json!({
            "success": true,
            "message": "连接成功（API 可达，模型或请求参数可能需要调整）",
            "status": status,
        }))),
        // 429: 限流（连接和认证正常，只是请求过于频繁）
        429 => Ok(Json(serde_json::json!({
            "success": true,
            "message": "连接成功（当前被限流，请稍后再正式使用）",
            "status": status,
        }))),
        // 其他: 服务端错误
        _ => {
            let error_text = response.text().await.unwrap_or_default();
            Err(Error::Internal(format!("服务端错误 ({}): {}", status, error_text)))
        }
    }
}

/// GET /api/client-models — 客户端拉取可用模型列表
///
/// 支持通过 user_id 查询参数按部门白名单过滤。
/// 未传 user_id 或用户无部门或部门无白名单时，返回所有已启用模型。
#[instrument(skip(state))]
async fn get_client_models(
    State(state): State<AppState>,
    Query(params): Query<ClientModelsQuery>,
) -> Result<Json<Vec<models::ClientModelConfig>>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    // 确定白名单过滤条件
    let whitelist_model_ids = resolve_whitelist(&client, params.user_id).await?;

    let rows = client
        .query(
            "SELECT mc.id, mc.model_id, mc.display_name, mc.description, mc.provider, mc.is_default, \
             mc.capabilities, mc.api_base_url, mc.api_key, COALESCE(mc.extra_config->>'api_format', 'openai') \
             FROM model_configs mc WHERE mc.enabled = true ORDER BY mc.sort_order, mc.display_name",
            &[],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let models: Vec<models::ClientModelConfig> = rows
        .iter()
        .filter(|row| {
            // 如果有白名单，只返回白名单中的模型
            match &whitelist_model_ids {
                Some(ids) => ids.contains(&row.get::<_, Uuid>(0)),
                None => true,
            }
        })
        .map(|row| {
            let caps_json: serde_json::Value = row.get(6);
            let capabilities = caps_json
                .as_array()
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();

            models::ClientModelConfig {
                model_id: row.get(1),
                display_name: row.get(2),
                description: row.get(3),
                provider: row.get(4),
                is_default: row.get(5),
                capabilities,
                api_base_url: row.get(7),
                api_key: row.get(8),
                api_format: row.get::<_, Option<String>>(9).unwrap_or_else(|| "openai".to_string()),
                source: "admin".to_string(),
            }
        })
        .collect();

    Ok(Json(models))
}

#[derive(Debug, Deserialize)]
struct ClientModelsQuery {
    user_id: Option<Uuid>,
}

/// 查询用户所属部门的模型白名单。
/// 返回 None 表示不过滤（无部门或部门无白名单）。
async fn resolve_whitelist(
    client: &deadpool_postgres::Object,
    user_id: Option<Uuid>,
) -> Result<Option<Vec<Uuid>>> {
    let uid = match user_id {
        Some(id) => id,
        None => return Ok(None),
    };

    // 查用户部门
    let dept_row = client.query_opt(
        "SELECT department_id FROM users WHERE id = $1",
        &[&uid],
    ).await.map_err(|e| Error::Database(e.to_string()))?;

    let dept_id: Option<Uuid> = dept_row.and_then(|r| r.get(0));
    let dept_id = match dept_id {
        Some(id) => id,
        None => return Ok(None),
    };

    // 查白名单
    let rows = client.query(
        "SELECT model_config_id FROM department_model_whitelist WHERE department_id = $1",
        &[&dept_id],
    ).await.map_err(|e| Error::Database(e.to_string()))?;

    if rows.is_empty() {
        return Ok(None); // 无白名单 = 不过滤
    }

    Ok(Some(rows.iter().map(|r| r.get(0)).collect()))
}

/// 规范化 API base URL：去掉 LLM SDK 会自动拼接的路径后缀。
///
/// 用户在 admin 后台输入 base URL 时可能带上完整路径（如
/// `https://api.example.com/v1/chat/completions`），但 rig-core 等 SDK
/// 会自动拼接 `/chat/completions`，导致路径重复。
/// 在保存到数据库前统一剥离，避免运行时出错。
fn normalize_api_base_url(url: &str) -> String {
    let trimmed = url.trim_end_matches('/');
    // 顺序重要：先匹配不含 /v1 的后缀，保留 /v1（它是 base URL 的一部分）。
    // 例如 https://api.openai.com/v1/chat/completions → https://api.openai.com/v1
    for suffix in &[
        "/chat/completions",
        "/completions",
        "/messages",
    ] {
        if let Some(base) = trimmed.strip_suffix(suffix) {
            return base.to_string();
        }
    }
    trimmed.to_string()
}
