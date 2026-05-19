//! Job state machine — the headless-framework vocabulary for "what is this
//! job doing right now?".
//!
//! Any dasclaw host (desktop GUI, admin-backend, claw-code, CLI, ...) needs
//! the same finite-state model: a job can be `Pending`, in progress, stuck,
//! completed, etc. Moving this vocabulary into the shared runtime crate is
//! step 1 of F3.2 phase 2 PR 3 (#641): later sub-PRs will add a
//! `JobContextCore` trait that exposes the framework-essential accessors
//! (job id, workspace, timeout, permission context) and split the current
//! ironclaw `JobContext` god-struct into "core fields" + GUI/marketplace
//! trait extension.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Lifecycle state of a job.
///
/// State transitions are validated by [`JobState::can_transition_to`]. The
/// transition graph is conservative: only forward / well-defined edges are
/// permitted, and `Completed -> Completed` is the single allowed self-loop
/// (it makes the worker-wrapper / execution-loop race a no-op instead of an
/// error that would mask a successful completion).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    /// Job is waiting to be started.
    Pending,
    /// Job is currently being worked on.
    InProgress,
    /// Job work is complete, awaiting submission.
    Completed,
    /// Job has been submitted for review.
    Submitted,
    /// Job was accepted/paid.
    Accepted,
    /// Job failed and cannot be completed.
    Failed,
    /// Job is stuck and needs repair.
    Stuck,
    /// Job was cancelled.
    Cancelled,
}

impl JobState {
    /// Check if this state allows transitioning to another state.
    pub fn can_transition_to(&self, target: JobState) -> bool {
        use JobState::*;

        // Allow idempotent Completed -> Completed transition.
        // Both the execution loop and the worker wrapper may race to mark a
        // job complete; the second call should be a harmless no-op rather
        // than an error that masks the successful completion.
        if matches!((self, target), (Completed, Completed)) {
            return true;
        }

        matches!(
            (self, target),
            // From Pending
            (Pending, InProgress) | (Pending, Cancelled) |
            // From InProgress
            (InProgress, Completed) | (InProgress, Failed) |
            (InProgress, Stuck) | (InProgress, Cancelled) |
            // From Completed
            (Completed, Submitted) | (Completed, Failed) |
            // From Submitted
            (Submitted, Accepted) | (Submitted, Failed) |
            // From Stuck (can recover or fail)
            (Stuck, InProgress) | (Stuck, Failed) | (Stuck, Cancelled)
        )
    }

    /// Check if this is a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Accepted | Self::Failed | Self::Cancelled)
    }

    /// Check if the job is active (not terminal).
    pub fn is_active(&self) -> bool {
        !self.is_terminal()
    }

    /// Check if this job consumes a parallel execution slot.
    ///
    /// Only jobs in Pending, InProgress, or Stuck states consume execution
    /// resources and should count toward the parallel job limit. Completed
    /// and Submitted jobs are in the state machine but are no longer
    /// actively executing.
    pub fn is_parallel_blocking(&self) -> bool {
        matches!(self, Self::Pending | Self::InProgress | Self::Stuck)
    }
}

impl std::fmt::Display for JobState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Pending => "pending",
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
            Self::Submitted => "submitted",
            Self::Accepted => "accepted",
            Self::Failed => "failed",
            Self::Stuck => "stuck",
            Self::Cancelled => "cancelled",
        };
        write!(f, "{s}")
    }
}

/// A recorded state transition event in a job's history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateTransition {
    /// Previous state.
    pub from: JobState,
    /// New state.
    pub to: JobState,
    /// When the transition occurred.
    pub timestamp: DateTime<Utc>,
    /// Reason for the transition.
    pub reason: Option<String>,
}

/// Error returned when a job exceeds its token budget.
///
/// Carries both the consumed total and the configured limit so callers can
/// emit precise diagnostics (e.g. channel response, audit trail) without
/// re-deriving the numbers.
#[derive(Debug, Error)]
#[error("Token budget exceeded: used {used} of {limit} allowed tokens")]
pub struct TokenBudgetExceeded {
    /// Total tokens consumed (including the call that exceeded the budget).
    pub used: u64,
    /// Configured token limit for this job.
    pub limit: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_dasclaw_runtime_job_state_transitions_forward_allowed() {
        assert!(JobState::Pending.can_transition_to(JobState::InProgress));
        assert!(JobState::InProgress.can_transition_to(JobState::Completed));
        assert!(JobState::Completed.can_transition_to(JobState::Submitted));
        assert!(JobState::Submitted.can_transition_to(JobState::Accepted));
    }

    #[test]
    fn req_dasclaw_runtime_job_state_transitions_backward_rejected() {
        assert!(!JobState::Completed.can_transition_to(JobState::Pending));
        assert!(!JobState::Accepted.can_transition_to(JobState::InProgress));
        assert!(!JobState::Cancelled.can_transition_to(JobState::Pending));
    }

    #[test]
    fn req_dasclaw_runtime_job_state_idempotent_completed_only() {
        // The single allowed self-loop: Completed -> Completed. Models the
        // worker-wrapper vs execution-loop race.
        assert!(JobState::Completed.can_transition_to(JobState::Completed));

        // All other self-loops are forbidden.
        assert!(!JobState::Pending.can_transition_to(JobState::Pending));
        assert!(!JobState::InProgress.can_transition_to(JobState::InProgress));
        assert!(!JobState::Failed.can_transition_to(JobState::Failed));
        assert!(!JobState::Stuck.can_transition_to(JobState::Stuck));
        assert!(!JobState::Submitted.can_transition_to(JobState::Submitted));
        assert!(!JobState::Accepted.can_transition_to(JobState::Accepted));
        assert!(!JobState::Cancelled.can_transition_to(JobState::Cancelled));
    }

    #[test]
    fn req_dasclaw_runtime_job_state_terminal_classification() {
        assert!(JobState::Accepted.is_terminal());
        assert!(JobState::Failed.is_terminal());
        assert!(JobState::Cancelled.is_terminal());
        // Active states
        assert!(!JobState::Pending.is_terminal());
        assert!(!JobState::InProgress.is_terminal());
        assert!(!JobState::Stuck.is_terminal());
        assert!(!JobState::Completed.is_terminal());
        assert!(!JobState::Submitted.is_terminal());
        // is_active is the strict complement
        assert!(!JobState::Accepted.is_active());
        assert!(JobState::Pending.is_active());
    }

    #[test]
    fn req_dasclaw_runtime_job_state_parallel_blocking_set() {
        // These three states reserve a parallel-execution slot.
        assert!(JobState::Pending.is_parallel_blocking());
        assert!(JobState::InProgress.is_parallel_blocking());
        assert!(JobState::Stuck.is_parallel_blocking());

        // Completed/Submitted are still alive but no longer hold a slot;
        // misclassifying them would deadlock the worker pool.
        assert!(!JobState::Completed.is_parallel_blocking());
        assert!(!JobState::Submitted.is_parallel_blocking());
        assert!(!JobState::Accepted.is_parallel_blocking());
        assert!(!JobState::Failed.is_parallel_blocking());
        assert!(!JobState::Cancelled.is_parallel_blocking());
    }

    #[test]
    fn req_dasclaw_runtime_job_state_display_lowercase_snake() {
        // Display output is the on-wire / log format; downstream channels
        // depend on it. snake_case keeps the rendering consistent with the
        // serde `rename_all = "snake_case"` directive.
        assert_eq!(JobState::Pending.to_string(), "pending");
        assert_eq!(JobState::InProgress.to_string(), "in_progress");
        assert_eq!(JobState::Stuck.to_string(), "stuck");
    }

    #[test]
    fn req_dasclaw_runtime_job_state_serde_round_trip_snake_case() {
        let json = serde_json::to_string(&JobState::InProgress).expect("serialize");
        assert_eq!(json, "\"in_progress\"");
        let back: JobState = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, JobState::InProgress);
    }

    #[test]
    fn req_dasclaw_runtime_state_transition_round_trip() {
        let t = StateTransition {
            from: JobState::Pending,
            to: JobState::InProgress,
            timestamp: chrono::DateTime::<Utc>::from_timestamp(1_700_000_000, 0).unwrap(),
            reason: Some("worker picked up".into()),
        };
        let json = serde_json::to_string(&t).expect("serialize");
        // Field names and snake-case state values must survive the round
        // trip — downstream consumers (web channels, audit log) rely on it.
        assert!(json.contains("\"from\":\"pending\""));
        assert!(json.contains("\"to\":\"in_progress\""));
        assert!(json.contains("worker picked up"));
        let back: StateTransition = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.from, JobState::Pending);
        assert_eq!(back.to, JobState::InProgress);
        assert_eq!(back.timestamp, t.timestamp);
        assert_eq!(back.reason.as_deref(), Some("worker picked up"));
    }

    #[test]
    fn req_dasclaw_runtime_token_budget_exceeded_display() {
        let err = TokenBudgetExceeded {
            used: 12_345,
            limit: 10_000,
        };
        assert_eq!(
            err.to_string(),
            "Token budget exceeded: used 12345 of 10000 allowed tokens"
        );
    }
}
