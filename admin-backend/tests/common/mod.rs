#![allow(dead_code)]

use admin_backend::{routes::create_router, AppState};
use axum::{
    body::Body,
    http::{Request, StatusCode},
    routing::post,
    Json, Router,
};
use deadpool_postgres::Config;
use http_body_util::BodyExt;
use serde_json::Value;
use std::net::SocketAddr;
use std::sync::{Mutex, MutexGuard};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio::time::{sleep, Duration};
use tokio_postgres::NoTls;
use tower::ServiceExt;

static SCANNER_ENV_LOCK: Mutex<()> = Mutex::new(());

pub struct ScannerEnvGuard {
    _lock: MutexGuard<'static, ()>,
    old_enabled: Option<String>,
    old_url: Option<String>,
    old_timeout: Option<String>,
}

impl Drop for ScannerEnvGuard {
    fn drop(&mut self) {
        restore_env_var("SCANNER_ENABLED", self.old_enabled.take());
        restore_env_var("SCANNER_URL", self.old_url.take());
        restore_env_var("SCANNER_TIMEOUT_MS", self.old_timeout.take());
    }
}

fn restore_env_var(key: &str, value: Option<String>) {
    if let Some(v) = value {
        std::env::set_var(key, v);
    } else {
        std::env::remove_var(key);
    }
}

pub fn configure_scanner_env(
    enabled: bool,
    url: Option<&str>,
    timeout_ms: Option<u64>,
) -> ScannerEnvGuard {
    let lock = SCANNER_ENV_LOCK.lock().expect("lock scanner env");
    let old_enabled = std::env::var("SCANNER_ENABLED").ok();
    let old_url = std::env::var("SCANNER_URL").ok();
    let old_timeout = std::env::var("SCANNER_TIMEOUT_MS").ok();

    std::env::set_var("SCANNER_ENABLED", if enabled { "true" } else { "false" });
    restore_env_var("SCANNER_URL", url.map(ToString::to_string));
    restore_env_var(
        "SCANNER_TIMEOUT_MS",
        timeout_ms.map(|v| v.to_string()),
    );

    ScannerEnvGuard {
        _lock: lock,
        old_enabled,
        old_url,
        old_timeout,
    }
}

pub async fn spawn_scanner_server(response: Value, delay_ms: u64) -> (String, JoinHandle<()>) {
    let app = Router::new().route(
        "/scan-upload",
        post(move || {
            let response = response.clone();
            async move {
                sleep(Duration::from_millis(delay_ms)).await;
                Json(response)
            }
        }),
    );

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test scanner server");
    let addr: SocketAddr = listener.local_addr().expect("get scanner addr");
    let handle = tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("serve scanner app");
    });

    (format!("http://{}", addr), handle)
}

pub async fn try_connect_db() -> Option<deadpool_postgres::Pool> {
    let mut cfg = Config::new();
    cfg.host = Some(std::env::var("DB_HOST").unwrap_or_else(|_| "localhost".to_string()));
    cfg.port = Some(5432);
    cfg.user = Some(std::env::var("DB_USER").unwrap_or_else(|_| "postgres".to_string()));
    cfg.password = Some(std::env::var("DB_PASSWORD").unwrap_or_else(|_| "postgres".to_string()));
    cfg.dbname = Some(std::env::var("DB_NAME").unwrap_or_else(|_| "ironclaw".to_string()));
    let pool = cfg.create_pool(None, NoTls).ok()?;
    let _ = pool.get().await.ok()?;
    Some(pool)
}

pub fn build_app(pool: deadpool_postgres::Pool) -> axum::Router {
    let state = AppState {
        db_pool: pool.clone(),
        sqlx_pool: {
            let db_url = format!(
                "postgres://{}:{}@{}:{}/{}",
                std::env::var("DB_USER").unwrap_or_else(|_| "postgres".to_string()),
                std::env::var("DB_PASSWORD").unwrap_or_else(|_| "postgres".to_string()),
                std::env::var("DB_HOST").unwrap_or_else(|_| "localhost".to_string()),
                5432,
                std::env::var("DB_NAME").unwrap_or_else(|_| "ironclaw".to_string()),
            );
            sqlx::PgPool::connect_lazy(&db_url).expect("lazy connect should not fail")
        },
        http_client: reqwest::Client::new(),
        gateway_url: "http://localhost:38080".to_string(),
    };
    create_router(state)
}

pub fn make_auth_token() -> String {
    let secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "secret".to_string());
    let auth = admin_backend::auth::AuthManager::new(secret);
    auth.generate_access_token("00000000-0000-0000-0000-000000000001")
        .expect("test token generation should not fail")
}

pub async fn get(app: axum::Router, path: &str) -> axum::response::Response {
    get_with_headers(app, path, &[]).await
}

pub async fn get_with_headers(
    app: axum::Router,
    path: &str,
    headers: &[(&str, &str)],
) -> axum::response::Response {
    let token = make_auth_token();
    let mut builder = Request::builder()
        .method("GET")
        .uri(path)
        .header("authorization", format!("Bearer {}", token));

    for (k, v) in headers {
        builder = builder.header(*k, *v);
    }

    app.oneshot(builder.body(Body::empty()).expect("build GET request"))
        .await
        .expect("execute GET request")
}

pub async fn post_json(
    app: axum::Router,
    path: &str,
    body: Value,
) -> axum::response::Response {
    post_json_with_headers(app, path, body, &[]).await
}

pub async fn post_json_with_headers(
    app: axum::Router,
    path: &str,
    body: Value,
    headers: &[(&str, &str)],
) -> axum::response::Response {
    let token = make_auth_token();
    let mut builder = Request::builder()
        .method("POST")
        .uri(path)
        .header("authorization", format!("Bearer {}", token))
        .header("content-type", "application/json");

    for (k, v) in headers {
        builder = builder.header(*k, *v);
    }

    app.oneshot(
        builder
            .body(Body::from(
                serde_json::to_vec(&body).expect("serialize POST body"),
            ))
            .expect("build POST request"),
    )
    .await
    .expect("execute POST request")
}

pub async fn response_json(resp: axum::response::Response) -> Value {
    let bytes = resp
        .into_body()
        .collect()
        .await
        .expect("read response body")
        .to_bytes();
    serde_json::from_slice(&bytes).expect("parse response json")
}

pub fn assert_status_ok(status: StatusCode) {
    assert_eq!(
        status,
        StatusCode::OK,
        "expected HTTP 200, got {}",
        status
    );
}

pub fn unique_name(prefix: &str) -> String {
    format!("{}_{}", prefix, uuid::Uuid::new_v4().simple())
}
