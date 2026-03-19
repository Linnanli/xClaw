use crate::auth::AuthManager;
use crate::db::Database;
use crate::error::{Error, Result};
use crate::handlers::{
    get_dlp_policies_handler, get_policies_handler, get_policy_version_handler,
    get_sensitive_ops_policies_handler,
};
use crate::models::{CreateUserRequest, LoginRequest, LoginResponse, RefreshTokenRequest};
use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
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
        .route("/api/users/{id}", get(get_user))
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
    State(state): State<AppState>,
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
