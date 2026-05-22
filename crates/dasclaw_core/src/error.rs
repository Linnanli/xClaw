//! Error types ported from `desktop-client/ironclaw/src/error.rs`.
//!
//! Verbatim port per ADR-152 §3 F3.6 slice 2/6 and ADR-129 §1.3. Only
//! `JobError` lives here for now; other error families remain in the host
//! crate until their respective slices land. The host's `crate::error::JobError`
//! is kept as a `pub use` re-export shim, matching the pattern already used by
//! [`dasclaw_workspace_cap::error::WorkspaceError`] and
//! [`dasclaw_runtime::ToolError`].

use std::time::Duration;

use uuid::Uuid;

/// Job-related errors.
#[derive(Debug, thiserror::Error)]
pub enum JobError {
    #[error("Job {id} not found")]
    NotFound { id: Uuid },

    #[error("Job {id} already in state {state}, cannot transition to {target}")]
    InvalidTransition {
        id: Uuid,
        state: String,
        target: String,
    },

    #[error("Job {id} failed: {reason}")]
    Failed { id: Uuid, reason: String },

    #[error("Job {id} stuck for {duration:?}")]
    Stuck { id: Uuid, duration: Duration },

    #[error("Maximum parallel jobs ({max}) exceeded")]
    MaxJobsExceeded { max: usize },

    #[error("Job {id} context error: {reason}")]
    ContextError { id: Uuid, reason: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_error_display() {
        let err = JobError::MaxJobsExceeded { max: 5 };
        let msg = err.to_string();
        assert!(msg.contains("5"), "Should mention max: {msg}");

        let id = Uuid::new_v4();
        let err = JobError::NotFound { id };
        let msg = err.to_string();
        assert!(
            msg.contains(&id.to_string()),
            "Should mention job id: {msg}"
        );
    }
}
