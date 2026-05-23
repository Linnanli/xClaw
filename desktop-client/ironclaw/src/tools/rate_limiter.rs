//! Shared rate limiter for built-in and WASM tool invocations.
//!
//! The implementation moved to [`dasclaw_runtime::rate_limit`] as part of
//! ADR-153 step 2 (headless agent runtime). This module re-exports the
//! types so existing ironclaw imports (`crate::tools::rate_limiter::*`)
//! keep working without churn.

pub use dasclaw_runtime::rate_limit::{LimitType, RateLimitError, RateLimitResult, RateLimiter};
