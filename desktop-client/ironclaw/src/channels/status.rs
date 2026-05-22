//! Desktop-side `StatusUpdate::tool_completed` builder.
//!
//! Lives here (not in `dasclaw_channels`) because it depends on desktop-only
//! types (`crate::error::Error`, `crate::tools::Tool`, `crate::tools::redact_params`).
//! Relocated from `dasclaw_channels::channel::StatusUpdate::tool_completed` per
//! F4.5 first slice.

use dasclaw_channels::StatusUpdate;

use crate::error::Error;
use crate::tools::{Tool, redact_params};

/// Build a `ToolCompleted` status with redacted parameters.
///
/// On failure, serializes the tool's input parameters as pretty JSON after
/// replacing any keys listed in the tool's `sensitive_params()` with
/// `"[REDACTED]"`. On success, no parameters or error are included.
pub fn build_tool_completed(
    name: String,
    result: &Result<String, Error>,
    params: &serde_json::Value,
    tool: Option<&dyn Tool>,
) -> StatusUpdate {
    let success = result.is_ok();
    let sensitive = tool.map(|t| t.sensitive_params()).unwrap_or(&[]);
    StatusUpdate::ToolCompleted {
        name,
        success,
        error: result.as_ref().err().map(|e| e.to_string()),
        parameters: if !success {
            let safe = redact_params(params, sensitive);
            Some(serde_json::to_string_pretty(&safe).unwrap_or_else(|_| safe.to_string()))
        } else {
            None
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::credentials::TEST_REDACT_SECRET_123;
    use async_trait::async_trait;
    use dasclaw_channels::StatusUpdate;

    /// Stub tool that marks `"value"` as sensitive.
    struct SecretTool;

    #[async_trait]
    impl crate::tools::Tool for SecretTool {
        fn name(&self) -> &str {
            "secret_save"
        }
        fn description(&self) -> &str {
            "stub"
        }
        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({"type": "object", "properties": {}})
        }
        async fn execute(
            &self,
            _params: serde_json::Value,
            _ctx: &mut dyn dasclaw_runtime::JobContextCore,
        ) -> Result<crate::tools::ToolOutput, crate::tools::ToolError> {
            unreachable!()
        }
        fn sensitive_params(&self) -> &[&str] {
            &["value"]
        }
    }

    #[test]
    fn tool_completed_redacts_sensitive_params_on_failure() {
        let params = serde_json::json!({"name": "api_key", "value": TEST_REDACT_SECRET_123});
        let err: Result<String, crate::error::Error> =
            Err(crate::error::ToolError::ExecutionFailed {
                name: "secret_save".into(),
                reason: "db error".into(),
            }
            .into());
        let tool = SecretTool;

        let status = build_tool_completed(
            "secret_save".into(),
            &err,
            &params,
            Some(&tool as &dyn crate::tools::Tool),
        );

        if let StatusUpdate::ToolCompleted {
            success,
            error,
            parameters,
            ..
        } = &status
        {
            assert!(!success);
            let err_msg = error.as_deref().expect("should have error");
            assert!(err_msg.contains("db error"), "error: {}", err_msg);
            let param_str = parameters
                .as_ref()
                .expect("should have parameters on failure");
            assert!(
                param_str.contains("[REDACTED]"),
                "sensitive value should be redacted: {}",
                param_str
            );
            assert!(
                !param_str.contains(TEST_REDACT_SECRET_123),
                "raw secret should not appear: {}",
                param_str
            );
            assert!(
                param_str.contains("api_key"),
                "non-sensitive params should be preserved: {}",
                param_str
            );
        } else {
            panic!("expected ToolCompleted variant");
        }
    }

    #[test]
    fn tool_completed_no_params_on_success() {
        let params = serde_json::json!({"name": "key", "value": "secret"});
        let ok: Result<String, crate::error::Error> = Ok("done".into());

        let status = build_tool_completed("secret_save".into(), &ok, &params, None);

        if let StatusUpdate::ToolCompleted {
            success,
            error,
            parameters,
            ..
        } = &status
        {
            assert!(success);
            assert!(error.is_none());
            assert!(parameters.is_none(), "no params should be sent on success");
        } else {
            panic!("expected ToolCompleted variant");
        }
    }

    #[test]
    fn tool_completed_no_tool_passes_params_unredacted() {
        let params = serde_json::json!({"cmd": "ls -la"});
        let err: Result<String, crate::error::Error> =
            Err(crate::error::ToolError::ExecutionFailed {
                name: "shell".into(),
                reason: "timeout".into(),
            }
            .into());

        let status = build_tool_completed("shell".into(), &err, &params, None);

        if let StatusUpdate::ToolCompleted { parameters, .. } = &status {
            let param_str = parameters.as_ref().expect("should have parameters");
            assert!(
                param_str.contains("ls -la"),
                "non-sensitive params should pass through: {}",
                param_str
            );
        } else {
            panic!("expected ToolCompleted variant");
        }
    }
}
