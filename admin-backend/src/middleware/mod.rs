pub mod auth;
pub mod rate_limit;
pub mod security_headers;

use axum::http::HeaderMap;

/// 从请求头提取客户端 IP。
///
/// 优先级：X-Forwarded-For 第一个地址 → X-Real-IP → None
pub fn extract_client_ip(headers: &HeaderMap) -> Option<String> {
    if let Some(val) = headers.get("x-forwarded-for") {
        if let Ok(s) = val.to_str() {
            let first = s.split(',').next().unwrap_or("").trim();
            if !first.is_empty() {
                return Some(first.to_owned());
            }
        }
    }
    if let Some(val) = headers.get("x-real-ip") {
        if let Ok(s) = val.to_str() {
            let s = s.trim();
            if !s.is_empty() {
                return Some(s.to_owned());
            }
        }
    }
    None
}
