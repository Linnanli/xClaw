//! Routine-related errors.
//!
//! Moved verbatim from `desktop-client/ironclaw/src/error.rs` (ADR-129 §1.3).

use uuid::Uuid;

/// Routine-related errors.
#[derive(Debug, thiserror::Error)]
pub enum RoutineError {
    #[error("Unknown trigger type: {trigger_type}")]
    UnknownTriggerType { trigger_type: String },

    #[error("Unknown action type: {action_type}")]
    UnknownActionType { action_type: String },

    #[error("Missing field in {context}: {field}")]
    MissingField { context: String, field: String },

    #[error("Invalid cron expression: {reason}")]
    InvalidCron { reason: String },

    #[error("Unknown run status: {status}")]
    UnknownRunStatus { status: String },

    #[error("Routine {name} is disabled")]
    Disabled { name: String },

    #[error("Routine not found: {id}")]
    NotFound { id: Uuid },

    #[error("Not authorized to trigger routine {id}")]
    NotAuthorized { id: Uuid },

    #[error("Routine {name} is in cooldown period")]
    Cooldown { name: String },

    #[error("Routine {name} at max concurrent runs")]
    MaxConcurrent { name: String },

    #[error("Database error: {reason}")]
    Database { reason: String },

    #[error("LLM call failed: {reason}")]
    LlmFailed { reason: String },

    #[error("Failed to dispatch full job: {reason}")]
    JobDispatchFailed { reason: String },

    #[error("LLM returned empty content")]
    EmptyResponse,

    #[error("LLM response truncated (finish_reason=length) with no content")]
    TruncatedResponse,
}
