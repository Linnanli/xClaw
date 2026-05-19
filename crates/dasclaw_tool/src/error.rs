//! Tool implementation-layer error type.
//!
//! Returned by an individual tool's `execute()` method. The error does not
//! carry the tool's name on purpose: the dispatcher that invoked the tool
//! attaches the identity when it converts this into the outer application
//! error (see future `dasclaw_runtime::ToolError`).

use std::time::Duration;

use thiserror::Error;

/// Error type for tool execution.
///
/// Variants intentionally use tuple-style payloads to keep tools' error
/// construction terse. The wrapping application error converts these into
/// rich struct-style variants that include the failing tool's identity.
#[derive(Debug, Error)]
pub enum ToolError {
    #[error("Invalid parameters: {0}")]
    InvalidParameters(String),

    #[error("Execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Timeout after {0:?}")]
    Timeout(Duration),

    #[error("Not authorized: {0}")]
    NotAuthorized(String),

    #[error("Rate limited, retry after {0:?}")]
    RateLimited(Option<Duration>),

    #[error("External service error: {0}")]
    ExternalService(String),

    #[error("Sandbox error: {0}")]
    Sandbox(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_dasclaw_tool_error_display_invalid_parameters() {
        let err = ToolError::InvalidParameters("missing name".into());
        assert_eq!(err.to_string(), "Invalid parameters: missing name");
    }

    #[test]
    fn req_dasclaw_tool_error_display_timeout() {
        let err = ToolError::Timeout(Duration::from_secs(5));
        assert_eq!(err.to_string(), "Timeout after 5s");
    }

    #[test]
    fn req_dasclaw_tool_error_display_rate_limited_some() {
        let err = ToolError::RateLimited(Some(Duration::from_millis(250)));
        assert_eq!(err.to_string(), "Rate limited, retry after Some(250ms)");
    }

    #[test]
    fn req_dasclaw_tool_error_display_rate_limited_none() {
        let err = ToolError::RateLimited(None);
        assert_eq!(err.to_string(), "Rate limited, retry after None");
    }

    #[test]
    fn req_dasclaw_tool_error_variants_distinct() {
        let variants = [
            ToolError::InvalidParameters("a".into()),
            ToolError::ExecutionFailed("b".into()),
            ToolError::NotAuthorized("c".into()),
            ToolError::ExternalService("d".into()),
            ToolError::Sandbox("e".into()),
        ];
        // Every variant's Display output must include its tag so dispatcher
        // logs can distinguish them without pattern matching on Debug.
        let messages: Vec<String> = variants.iter().map(|e| e.to_string()).collect();
        assert!(messages[0].contains("Invalid parameters"));
        assert!(messages[1].contains("Execution failed"));
        assert!(messages[2].contains("Not authorized"));
        assert!(messages[3].contains("External service error"));
        assert!(messages[4].contains("Sandbox error"));
    }
}
