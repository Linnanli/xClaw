//! HTTP recording / replay vocabulary shared across dasclaw hosts.
//!
//! These types and the [`HttpInterceptor`] trait were verbatim-ported from
//! `desktop-client/ironclaw/src/llm/recording.rs` (ADR-129 §1.3, ADR-154
//! step 1/2). The concrete `RecordingHttpInterceptor` and
//! `ReplayingHttpInterceptor` implementations stay in ironclaw — only the
//! trait + the request/response/exchange data carriers move down so that
//! the future [`crate::job_context::JobContextCore`] trait can expose
//! `Option<Arc<dyn HttpInterceptor>>` without dragging ironclaw down with
//! it.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// A single HTTP request/response pair captured during a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpExchange {
    pub request: HttpExchangeRequest,
    pub response: HttpExchangeResponse,
}

/// The request side of an HTTP exchange.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpExchangeRequest {
    pub method: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub headers: Vec<(String, String)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

/// The response side of an HTTP exchange.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpExchangeResponse {
    pub status: u16,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub headers: Vec<(String, String)>,
    pub body: String,
}

/// Trait for intercepting HTTP requests from tools.
///
/// During recording, the interceptor captures exchanges after the real
/// request completes. During replay, it short-circuits with a recorded response.
#[async_trait]
pub trait HttpInterceptor: Send + Sync + std::fmt::Debug {
    /// Called before making an HTTP request.
    ///
    /// Return `Some(response)` to short-circuit (replay mode).
    /// Return `None` to let the real request proceed (recording mode).
    async fn before_request(&self, request: &HttpExchangeRequest) -> Option<HttpExchangeResponse>;

    /// Called after a real HTTP request completes (recording mode only).
    async fn after_response(&self, request: &HttpExchangeRequest, response: &HttpExchangeResponse);
}
