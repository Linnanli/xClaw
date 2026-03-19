use crate::auth::AuthManager;
use crate::error::{Error, Result};
use crate::handlers::{
    get_dlp_policies_handler, get_policies_handler, get_policy_version_handler,
    get_sensitive_ops_policies_handler,
};
use crate::models::{CreateUserRequest, LoginRequest, LoginResponse, RefreshTokenRequest, CreateRoleRequest, UpdateRoleRequest, AssignPermissionsRequest};
use crate::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use serde_json::json;
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
        .route("/api/audit-logs", get(get_audit_logs))
        .route("/api/dlp-rules", get(get_dlp_rules))
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

async fn get_audit_logs(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let rows = client
        .query(
            "SELECT id, user_id, action, details, created_at FROM audit_logs ORDER BY created_at DESC LIMIT 100",
            &[],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let logs: Vec<_> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<_, Uuid>(0),
                "user_id": r.get::<_, Uuid>(1),
                "action": r.get::<_, String>(2),
                "details": r.get::<_, String>(3),
                "created_at": r.get::<_, chrono::DateTime<chrono::Utc>>(4),
            })
        })
        .collect();

    Ok(Json(json!({ "logs": logs })))
}

async fn get_dlp_rules(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    let rows = client
        .query(
            "SELECT id, pattern, replacement, severity, created_at FROM dlp_rules",
            &[],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    let rules: Vec<_> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<_, Uuid>(0),
                "pattern": r.get::<_, String>(1),
                "replacement": r.get::<_, String>(2),
                "severity": r.get::<_, String>(3),
                "created_at": r.get::<_, chrono::DateTime<chrono::Utc>>(4),
            })
        })
        .collect();

    Ok(Json(json!({ "rules": rules })))
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
