//! 基于 IP + 路径分组的滑动窗口限流中间件
//!
//! 限流分组：
//! - `/api/auth/login`：每 IP 每分钟最多 30 次（防暴力破解，不影响正常使用）
//! - 其他接口：每 IP 每分钟最多 300 次
//!
//! key = `{ip}:{group}`，登录和普通接口独立计数，互不影响。
//!
//! 可通过环境变量覆盖：
//! - `RATE_LIMIT_LOGIN`：登录接口限制
//! - `RATE_LIMIT_DEFAULT`：其他接口限制

use crate::middleware::extract_client_ip;
use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
};
use serde_json::json;
use std::{
    collections::{HashMap, VecDeque},
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tower::{Layer, Service};

const WINDOW: Duration = Duration::from_secs(60);
const LOGIN_LIMIT: usize = 30;
const DEFAULT_LIMIT: usize = 300;

/// 启动时读取一次限流配置，避免每次请求都读环境变量
fn get_limits() -> (usize, usize) {
    let login = std::env::var("RATE_LIMIT_LOGIN")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(LOGIN_LIMIT);
    let default = std::env::var("RATE_LIMIT_DEFAULT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_LIMIT);
    (login, default)
}

/// key = `{ip}:{group}`，不同分组独立计数
type WindowMap = Arc<Mutex<HashMap<String, VecDeque<Instant>>>>;

#[derive(Clone)]
pub struct RateLimitLayer {
    windows: WindowMap,
    login_limit: usize,
    default_limit: usize,
}

impl RateLimitLayer {
    pub fn new() -> Self {
        let (login_limit, default_limit) = get_limits();
        Self {
            windows: Arc::new(Mutex::new(HashMap::new())),
            login_limit,
            default_limit,
        }
    }
}

impl Default for RateLimitLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> Layer<S> for RateLimitLayer {
    type Service = RateLimitMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RateLimitMiddleware {
            inner,
            windows: self.windows.clone(),
            login_limit: self.login_limit,
            default_limit: self.default_limit,
        }
    }
}

#[derive(Clone)]
pub struct RateLimitMiddleware<S> {
    inner: S,
    windows: WindowMap,
    login_limit: usize,
    default_limit: usize,
}

impl<S> Service<Request<Body>> for RateLimitMiddleware<S>
where
    S: Service<Request<Body>, Response = Response> + Send + Clone + 'static,
    S::Future: Send + 'static,
{
    type Response = Response;
    type Error = S::Error;
    type Future =
        std::pin::Pin<Box<dyn std::future::Future<Output = Result<Response, S::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let path = req.uri().path();
        let is_login = path == "/api/auth/login";
        let limit = if is_login {
            self.login_limit
        } else {
            self.default_limit
        };

        let ip = extract_request_ip(&req);
        let group = if is_login { "login" } else { "default" };
        let key = format!("{}:{}", ip, group);

        let now = Instant::now();
        let allowed = {
            let mut map = self.windows.lock().expect("rate limit mutex poisoned");
            let queue = map.entry(key).or_default();

            while queue
                .front()
                .map(|t| now.duration_since(*t) >= WINDOW)
                .unwrap_or(false)
            {
                queue.pop_front();
            }

            if queue.len() < limit {
                queue.push_back(now);
                true
            } else {
                false
            }
        };

        if !allowed {
            let group_label = if is_login { "login" } else { "api" };
            tracing::warn!(
                ip = %ip,
                path = %path,
                group = %group_label,
                limit = %limit,
                "rate limit exceeded"
            );
            let body = axum::Json(json!({"error": "Too Many Requests"}));
            let resp = (StatusCode::TOO_MANY_REQUESTS, body).into_response();
            return Box::pin(async move { Ok(resp) });
        }

        let mut inner = self.inner.clone();
        Box::pin(async move { inner.call(req).await })
    }
}

fn extract_request_ip(req: &Request<Body>) -> String {
    if let Some(ip) = extract_client_ip(req.headers()) {
        return ip;
    }
    if let Some(addr) = req.extensions().get::<ConnectInfo<SocketAddr>>() {
        return addr.0.ip().to_string();
    }
    "unknown".to_owned()
}
