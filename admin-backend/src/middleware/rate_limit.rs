//! 基于 IP 的滑动窗口限流中间件
//!
//! - `/api/auth/login`：每分钟最多 10 次
//! - 其他接口：每分钟最多 100 次
//! - 超限返回 429 `{"error": "Too Many Requests"}`

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
const LOGIN_LIMIT: usize = 10;
const DEFAULT_LIMIT: usize = 100;

/// 每个 IP 的请求时间戳队列
type WindowMap = Arc<Mutex<HashMap<String, VecDeque<Instant>>>>;

#[derive(Clone)]
pub struct RateLimitLayer {
    windows: WindowMap,
}

impl RateLimitLayer {
    pub fn new() -> Self {
        Self {
            windows: Arc::new(Mutex::new(HashMap::new())),
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
        }
    }
}

#[derive(Clone)]
pub struct RateLimitMiddleware<S> {
    inner: S,
    windows: WindowMap,
}

impl<S> Service<Request<Body>> for RateLimitMiddleware<S>
where
    S: Service<Request<Body>, Response = Response> + Send + Clone + 'static,
    S::Future: Send + 'static,
{
    type Response = Response;
    type Error = S::Error;
    type Future = std::pin::Pin<Box<dyn std::future::Future<Output = Result<Response, S::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let path = req.uri().path().to_owned();
        let limit = if path == "/api/auth/login" {
            LOGIN_LIMIT
        } else {
            DEFAULT_LIMIT
        };

        let ip = extract_request_ip(&req);

        let now = Instant::now();
        let allowed = {
            let mut map = self.windows.lock().expect("rate limit mutex poisoned");
            let queue = map.entry(ip).or_default();

            // 清除窗口外的旧记录
            while queue.front().map(|t| now.duration_since(*t) >= WINDOW).unwrap_or(false) {
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
            let body = axum::Json(json!({"error": "Too Many Requests"}));
            let resp = (StatusCode::TOO_MANY_REQUESTS, body).into_response();
            return Box::pin(async move { Ok(resp) });
        }

        let mut inner = self.inner.clone();
        Box::pin(async move { inner.call(req).await })
    }
}

/// 从请求中提取客户端 IP，优先使用共享的 header 提取逻辑，
/// 最后 fallback 到 ConnectInfo（axum 注入的连接地址）。
fn extract_request_ip(req: &Request<Body>) -> String {
    if let Some(ip) = extract_client_ip(req.headers()) {
        return ip;
    }
    if let Some(addr) = req.extensions().get::<ConnectInfo<SocketAddr>>() {
        return addr.0.ip().to_string();
    }
    "unknown".to_owned()
}
