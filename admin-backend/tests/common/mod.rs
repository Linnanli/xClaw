#![allow(dead_code)]

use admin_backend::{routes::create_router, AppState};
use axum::{
    body::{Body, Bytes},
    http::{HeaderMap, Request, StatusCode},
    routing::{get as route_get, post},
    Json, Router,
};
use deadpool_postgres::Config;
use http_body_util::BodyExt;
use serde_json::json;
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
    let lock = SCANNER_ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let old_enabled = std::env::var("SCANNER_ENABLED").ok();
    let old_url = std::env::var("SCANNER_URL").ok();
    let old_timeout = std::env::var("SCANNER_TIMEOUT_MS").ok();

    std::env::set_var("SCANNER_ENABLED", if enabled { "true" } else { "false" });
    restore_env_var("SCANNER_URL", url.map(ToString::to_string));
    restore_env_var("SCANNER_TIMEOUT_MS", timeout_ms.map(|v| v.to_string()));

    ScannerEnvGuard {
        _lock: lock,
        old_enabled,
        old_url,
        old_timeout,
    }
}

pub async fn spawn_scanner_server(response: Value, delay_ms: u64) -> (String, JoinHandle<()>) {
    let app = Router::new()
        .route(
            "/scan-upload",
            post(move || {
                let response = response.clone();
                async move {
                    sleep(Duration::from_millis(delay_ms)).await;
                    Json(response)
                }
            }),
        )
        .route("/health", route_get(|| async { StatusCode::OK }));

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test scanner server");
    let addr: SocketAddr = listener.local_addr().expect("get scanner addr");
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve scanner app");
    });

    (format!("http://{}", addr), handle)
}

pub async fn spawn_scanner_server_with_http_error(
    status: StatusCode,
    body: &str,
) -> (String, JoinHandle<()>) {
    let body_text = body.to_string();
    let app = Router::new()
        .route(
            "/scan-upload",
            post(move || {
                let body_text = body_text.clone();
                async move { (status, body_text) }
            }),
        )
        .route("/health", route_get(|| async { StatusCode::OK }));

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test scanner server");
    let addr: SocketAddr = listener.local_addr().expect("get scanner addr");
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve scanner app");
    });

    (format!("http://{}", addr), handle)
}

pub async fn spawn_scanner_server_validating_llm(
    expected_provider: &str,
    expected_api_key: &str,
) -> (String, JoinHandle<()>) {
    spawn_scanner_server_validating_llm_with_options(
        expected_provider,
        expected_api_key,
        None,
        None,
        None,
    )
    .await
}

fn multipart_contains_field(body_text: &str, field_name: &str, expected_value: &str) -> bool {
    body_text.contains(&format!("name=\"{}\"", field_name))
        && body_text.contains(&format!("\r\n{}\r\n", expected_value))
}

fn header_equals(headers: &HeaderMap, header_name: &str, expected: Option<&str>) -> bool {
    match expected {
        Some(value) => headers
            .get(header_name)
            .and_then(|raw| raw.to_str().ok())
            .map(|actual| actual == value)
            .unwrap_or(false),
        None => true,
    }
}

pub async fn spawn_scanner_server_validating_llm_with_options(
    expected_provider: &str,
    expected_api_key: &str,
    expected_model: Option<&str>,
    expected_base_url: Option<&str>,
    expected_api_version: Option<&str>,
) -> (String, JoinHandle<()>) {
    let provider = expected_provider.to_string();
    let api_key = expected_api_key.to_string();
    let model = expected_model.map(str::to_string);
    let base_url = expected_base_url.map(str::to_string);
    let api_version = expected_api_version.map(str::to_string);

    let app = Router::new()
        .route(
            "/scan-upload",
            post(move |headers: HeaderMap, body: Bytes| {
                let provider = provider.clone();
                let api_key = api_key.clone();
                let model = model.clone();
                let base_url = base_url.clone();
                let api_version = api_version.clone();
                async move {
                    let body_text = String::from_utf8_lossy(&body);
                    let has_use_llm = body_text.contains("name=\"use_llm\"")
                        && body_text.contains("\r\ntrue\r\n");
                    let has_expected_provider =
                        multipart_contains_field(&body_text, "llm_provider", &provider);
                    let has_expected_api_key = headers
                        .get("X-LLM-Key")
                        .and_then(|value| value.to_str().ok())
                        .map(|value| value == api_key)
                        .unwrap_or(false);
                    let has_expected_model =
                        header_equals(&headers, "X-LLM-Model", model.as_deref());
                    let has_expected_base_url =
                        header_equals(&headers, "X-LLM-Base-URL", base_url.as_deref());
                    let has_expected_api_version =
                        header_equals(&headers, "X-LLM-API-Version", api_version.as_deref());

                    if has_use_llm
                        && has_expected_provider
                        && has_expected_api_key
                        && has_expected_model
                        && has_expected_base_url
                        && has_expected_api_version
                    {
                        Json(json!({
                            "scanner_type": "integration-scanner",
                            "verdict": "SAFE",
                            "is_safe": true,
                            "findings_count": 0,
                            "findings": [],
                            "scan_duration_ms": 5
                        }))
                    } else {
                        Json(json!({
                            "scanner_type": "integration-scanner",
                            "verdict": "BLOCKED",
                            "is_safe": false,
                            "findings_count": 1,
                            "findings": [{"rule_id": "IT_EXPECT_LLM", "severity": "HIGH"}],
                            "scan_duration_ms": 5
                        }))
                    }
                }
            }),
        )
        .route("/health", route_get(|| async { StatusCode::OK }));

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test scanner server");
    let addr: SocketAddr = listener.local_addr().expect("get scanner addr");
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve scanner app");
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

pub async fn post_json(app: axum::Router, path: &str, body: Value) -> axum::response::Response {
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

pub async fn put_json(app: axum::Router, path: &str, body: Value) -> axum::response::Response {
    let token = make_auth_token();
    let builder = Request::builder()
        .method("PUT")
        .uri(path)
        .header("authorization", format!("Bearer {}", token))
        .header("content-type", "application/json");

    app.oneshot(
        builder
            .body(Body::from(
                serde_json::to_vec(&body).expect("serialize PUT body"),
            ))
            .expect("build PUT request"),
    )
    .await
    .expect("execute PUT request")
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
    assert_eq!(status, StatusCode::OK, "expected HTTP 200, got {}", status);
}

pub fn unique_name(prefix: &str) -> String {
    format!("{}_{}", prefix, uuid::Uuid::new_v4().simple())
}
