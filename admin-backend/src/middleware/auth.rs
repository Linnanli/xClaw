//! JWT 认证中间件
//!
//! 保护所有 `/api/*` 路由（除 `/api/auth/*`、`/api/client-reports`、`/api/client-config`、
//! `/api/client-models`、`/api/policies*`、`/api/quota/check`、`/api/quota/report-usage`）。
//!
//! 验证通过后将 `TokenClaims` 注入 `Extension`，handler 可通过
//! `Extension(claims): Extension<TokenClaims>` 提取当前用户信息。

use axum::{
    body::Body,
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Json, Response},
};
use serde_json::json;

use crate::auth::AuthManager;

/// 不需要 JWT 验证的路径前缀
const PUBLIC_PREFIXES: &[&str] = &[
    "/api/auth/",
    "/api/client-reports",
    "/api/client-config",
    "/api/client-models",
    "/api/policies",
    "/api/quota/check",
    "/api/quota/report-usage",
    "/health",
];

pub async fn jwt_auth(request: Request<Body>, next: Next) -> Response {
    let path = request.uri().path();

    if PUBLIC_PREFIXES
        .iter()
        .any(|prefix| path.starts_with(prefix))
    {
        return next.run(request).await;
    }

    let token = match extract_bearer_token(request.headers()) {
        Some(t) => t,
        None => return unauthorized("Missing or invalid Authorization header"),
    };

    let jwt_secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "secret".to_string());
    let auth = AuthManager::new(jwt_secret);

    match auth.verify_token(&token) {
        Ok(claims) => {
            let mut request = request;
            request.extensions_mut().insert(claims);
            next.run(request).await
        }
        Err(_) => unauthorized("Invalid or expired token"),
    }
}

fn extract_bearer_token(headers: &axum::http::HeaderMap) -> Option<String> {
    let value = headers.get("authorization")?.to_str().ok()?;
    value.strip_prefix("Bearer ").map(|s| s.to_owned())
}

fn unauthorized(message: &str) -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({ "error": "Unauthorized", "details": message })),
    )
        .into_response()
}
