//! 安全 HTTP 头中间件
//!
//! 为所有响应注入：
//! - `Content-Security-Policy: default-src 'self'`
//! - `X-Frame-Options: DENY`
//! - `X-Content-Type-Options: nosniff`

use axum::{body::Body, http::Request, response::Response};
use std::future::Future;
use std::pin::Pin;
use tower::{Layer, Service};

#[derive(Clone, Default)]
pub struct SecurityHeadersLayer;

impl<S> Layer<S> for SecurityHeadersLayer {
    type Service = SecurityHeadersMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        SecurityHeadersMiddleware { inner }
    }
}

#[derive(Clone)]
pub struct SecurityHeadersMiddleware<S> {
    inner: S,
}

impl<S> Service<Request<Body>> for SecurityHeadersMiddleware<S>
where
    S: Service<Request<Body>, Response = Response> + Send + Clone + 'static,
    S::Future: Send + 'static,
{
    type Response = Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Response, S::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let mut inner = self.inner.clone();
        Box::pin(async move {
            let mut resp = inner.call(req).await?;
            let headers = resp.headers_mut();
            headers.insert(
                "content-security-policy",
                "default-src 'self'"
                    .parse()
                    .expect("valid CSP header value"),
            );
            headers.insert(
                "x-frame-options",
                "DENY".parse().expect("valid X-Frame-Options value"),
            );
            headers.insert(
                "x-content-type-options",
                "nosniff"
                    .parse()
                    .expect("valid X-Content-Type-Options value"),
            );
            Ok(resp)
        })
    }
}
