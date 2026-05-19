//! Application-layer tool error type.
//!
//! See the crate-level docs for why this is separate from
//! [`dasclaw_tool::ToolError`].

use std::time::Duration;

use thiserror::Error;

/// Error returned by the tool dispatcher to the rest of the application.
///
/// Every variant that originates from a specific tool carries that tool's
/// `name` so logs, traces and user-facing channel responses can attribute
/// the failure correctly. The single tuple variant `BuilderFailed` describes
/// failures of the tool **builder** subsystem (which is not itself a tool)
/// and therefore has no tool name.
#[derive(Debug, Error)]
pub enum ToolError {
    #[error("Tool {name} not found")]
    NotFound { name: String },

    #[error("Tool {name} execution failed: {reason}")]
    ExecutionFailed { name: String, reason: String },

    #[error("Tool {name} timed out after {timeout:?}")]
    Timeout { name: String, timeout: Duration },

    #[error("Invalid parameters for tool {name}: {reason}")]
    InvalidParameters { name: String, reason: String },

    #[error("Tool {name} is disabled: {reason}")]
    Disabled { name: String, reason: String },

    #[error("Sandbox error for tool {name}: {reason}")]
    Sandbox { name: String, reason: String },

    #[error("Tool {name} requires authentication")]
    AuthRequired { name: String },

    #[error("Tool {name} is not available for autonomous execution: {reason}")]
    AutonomousUnavailable { name: String, reason: String },

    #[error("Tool {name} is rate limited, retry after {retry_after:?}")]
    RateLimited {
        name: String,
        retry_after: Option<Duration>,
    },

    #[error("Tool builder failed: {0}")]
    BuilderFailed(String),
}

impl ToolError {
    /// Convert a tool implementation-layer error
    /// ([`dasclaw_tool::ToolError`]) into the application-layer error by
    /// attaching the failing tool's name.
    ///
    /// This is the *only* supported way to lift the inner error: a blanket
    /// `From<dasclaw_tool::ToolError>` is intentionally not provided, because
    /// it would have to fabricate a placeholder name and silently lose the
    /// real identity at the conversion boundary.
    ///
    /// # Mapping
    ///
    /// | Inner variant                  | Outer variant                                                   |
    /// |--------------------------------|-----------------------------------------------------------------|
    /// | `InvalidParameters(reason)`    | `InvalidParameters { name, reason }`                            |
    /// | `ExecutionFailed(reason)`      | `ExecutionFailed { name, reason }`                              |
    /// | `Timeout(d)`                   | `Timeout { name, timeout: d }`                                  |
    /// | `NotAuthorized(reason)`        | `ExecutionFailed { name, reason: "not authorized: {reason}" }`  |
    /// | `RateLimited(retry_after)`     | `RateLimited { name, retry_after }`                             |
    /// | `ExternalService(reason)`      | `ExecutionFailed { name, reason: "external service: {reason}" }`|
    /// | `Sandbox(reason)`              | `Sandbox { name, reason }`                                      |
    ///
    /// `NotAuthorized` is routed to `ExecutionFailed` (not `AuthRequired`)
    /// because the runtime variant `AuthRequired { name }` carries no reason
    /// field; folding the reason into `ExecutionFailed` preserves the
    /// originating message instead of dropping it.
    pub fn from_tool_impl(name: impl Into<String>, err: dasclaw_tool::ToolError) -> Self {
        let name = name.into();
        match err {
            dasclaw_tool::ToolError::InvalidParameters(reason) => {
                ToolError::InvalidParameters { name, reason }
            }
            dasclaw_tool::ToolError::ExecutionFailed(reason) => {
                ToolError::ExecutionFailed { name, reason }
            }
            dasclaw_tool::ToolError::Timeout(timeout) => ToolError::Timeout { name, timeout },
            dasclaw_tool::ToolError::NotAuthorized(reason) => ToolError::ExecutionFailed {
                name,
                reason: format!("not authorized: {reason}"),
            },
            dasclaw_tool::ToolError::RateLimited(retry_after) => {
                ToolError::RateLimited { name, retry_after }
            }
            dasclaw_tool::ToolError::ExternalService(reason) => ToolError::ExecutionFailed {
                name,
                reason: format!("external service: {reason}"),
            },
            dasclaw_tool::ToolError::Sandbox(reason) => ToolError::Sandbox { name, reason },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_dasclaw_runtime_error_display_not_found() {
        let err = ToolError::NotFound {
            name: "fs.read".into(),
        };
        assert_eq!(err.to_string(), "Tool fs.read not found");
    }

    #[test]
    fn req_dasclaw_runtime_error_display_execution_failed() {
        let err = ToolError::ExecutionFailed {
            name: "fs.read".into(),
            reason: "permission denied".into(),
        };
        assert_eq!(
            err.to_string(),
            "Tool fs.read execution failed: permission denied"
        );
    }

    #[test]
    fn req_dasclaw_runtime_error_display_builder_failed() {
        let err = ToolError::BuilderFailed("LLM unreachable".into());
        assert_eq!(err.to_string(), "Tool builder failed: LLM unreachable");
    }

    #[test]
    fn req_dasclaw_runtime_error_from_tool_impl_invalid_parameters() {
        let inner = dasclaw_tool::ToolError::InvalidParameters("missing path".into());
        let outer = ToolError::from_tool_impl("fs.read", inner);
        match outer {
            ToolError::InvalidParameters { name, reason } => {
                assert_eq!(name, "fs.read");
                assert_eq!(reason, "missing path");
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn req_dasclaw_runtime_error_from_tool_impl_timeout_preserves_duration() {
        let inner = dasclaw_tool::ToolError::Timeout(Duration::from_secs(7));
        let outer = ToolError::from_tool_impl("net.fetch", inner);
        match outer {
            ToolError::Timeout { name, timeout } => {
                assert_eq!(name, "net.fetch");
                assert_eq!(timeout, Duration::from_secs(7));
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn req_dasclaw_runtime_error_from_tool_impl_rate_limited_preserves_retry_after() {
        let inner = dasclaw_tool::ToolError::RateLimited(Some(Duration::from_millis(500)));
        let outer = ToolError::from_tool_impl("net.fetch", inner);
        match outer {
            ToolError::RateLimited { name, retry_after } => {
                assert_eq!(name, "net.fetch");
                assert_eq!(retry_after, Some(Duration::from_millis(500)));
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn req_dasclaw_runtime_error_from_tool_impl_not_authorized_routes_to_execution_failed() {
        // NotAuthorized must NOT silently become AuthRequired (which would
        // drop the reason). It is routed to ExecutionFailed so the message
        // survives the boundary crossing.
        let inner = dasclaw_tool::ToolError::NotAuthorized("missing scope: read".into());
        let outer = ToolError::from_tool_impl("net.fetch", inner);
        match outer {
            ToolError::ExecutionFailed { name, reason } => {
                assert_eq!(name, "net.fetch");
                assert!(
                    reason.contains("not authorized"),
                    "reason should be tagged: {reason}"
                );
                assert!(
                    reason.contains("missing scope: read"),
                    "original reason must be preserved: {reason}"
                );
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn req_dasclaw_runtime_error_from_tool_impl_external_service_preserves_reason() {
        let inner = dasclaw_tool::ToolError::ExternalService("503 from upstream".into());
        let outer = ToolError::from_tool_impl("net.fetch", inner);
        match outer {
            ToolError::ExecutionFailed { name, reason } => {
                assert_eq!(name, "net.fetch");
                assert!(reason.contains("external service"));
                assert!(reason.contains("503 from upstream"));
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn req_dasclaw_runtime_error_from_tool_impl_sandbox_passthrough() {
        let inner = dasclaw_tool::ToolError::Sandbox("seccomp denied openat".into());
        let outer = ToolError::from_tool_impl("fs.read", inner);
        match outer {
            ToolError::Sandbox { name, reason } => {
                assert_eq!(name, "fs.read");
                assert_eq!(reason, "seccomp denied openat");
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn req_dasclaw_runtime_error_security_explicit_name_required() {
        // Defence-in-depth: the *only* way to lift a tool-impl error into the
        // app layer is `from_tool_impl(name, err)`, which forces the caller
        // to pass a name. There is no `Default` impl and no blanket
        // `From<dasclaw_tool::ToolError>` impl. Adding either would let the
        // tool name silently fall back to "<unknown>" — a fail-open that
        // would break attribution in logs / channel responses.
        //
        // This test pins the *happy path* of the explicit constructor;
        // accidental introduction of a no-name fallback would be caught by
        // reviewers via the absence of these impls in the public API.
        let inner = dasclaw_tool::ToolError::Sandbox("guard".into());
        let outer = ToolError::from_tool_impl("explicit", inner);
        assert!(
            outer.to_string().contains("explicit"),
            "tool name must appear in Display output: {outer}"
        );
    }
}
