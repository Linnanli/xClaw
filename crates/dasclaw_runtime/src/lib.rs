//! Application-layer runtime types shared across dasclaw hosts (ironclaw,
//! admin-backend, claw-code, ...).
//!
//! ## Modules
//!
//! - [`error`] — application-layer `ToolError` carrying the failing tool's
//!   identity (F3.2 phase 2 PR 2).
//! - [`job`] — pure job state machine (`JobState`, `StateTransition`,
//!   `TokenBudgetExceeded`); the headless-framework vocabulary for "what is
//!   this job doing right now?" (F3.2 phase 2 PR 3).
//! - [`secrets`] — secret-management vocabulary: pure data types
//!   (`Secret`, `SecretRef`, `DecryptedSecret`, `SecretError`,
//!   `CreateSecretParams`, `CredentialLocation`, `CredentialMapping`) and
//!   the `SecretsStore` trait. Concrete backends (Postgres, LibSQL,
//!   in-memory, OS keychain) stay in ironclaw and migrate in follow-up
//!   sub-PRs 4b/4c (F3.2 phase 2 PR 4).
//!
//! ## Two-layer `ToolError` model
//!
//! There are *two* `ToolError` types in the codebase and that is intentional:
//!
//! - [`dasclaw_tool::ToolError`] is the **tool implementation layer** error.
//!   It is what an individual tool's `execute()` returns. It carries no tool
//!   name — the tool does not need to repeat its own identity.
//! - [`ToolError`] (this crate) is the **application / dispatcher layer**
//!   error. The dispatcher knows *which* tool failed, so every variant here
//!   carries the failing tool's `name` (and any additional runtime context
//!   such as `retry_after` for rate limiting).
//!
//! The conversion from the implementation layer to the application layer is
//! intentionally **not** a blanket [`From`] impl: a free `From` could not
//! supply the tool name and would silently fall back to `"<unknown>"` — a
//! fail-open behaviour incompatible with the security stance of x-claw.
//! Instead, callers must attach the name explicitly at the boundary using
//! [`ToolError::from_tool_impl`].

pub mod agent;
pub mod composite_executor;
pub mod context;
pub mod error;
pub mod feature_flags;
pub mod job;
pub mod job_context;
pub mod llm_adapter;
pub mod rate_limit;
pub mod recording;
pub mod secrets;
pub mod tool;
pub mod tool_to_executor_adapter;

pub use agent::{
    Agent, AgentBuilder, AgentConfig, AgentError, AgentResponder, ToolExecutor, ToolOutputSanitizer,
};
pub use composite_executor::{CompositeError, CompositeToolExecutor};
pub use error::ToolError;
pub use feature_flags::{SharedFeatureFlags, ToolFeatureFlags};
pub use job::{JobState, StateTransition, TokenBudgetExceeded};
pub use job_context::JobContextCore;
pub use llm_adapter::LlmProviderResponder;
pub use rate_limit::{LimitType, RateLimitError, RateLimitResult, RateLimiter};
pub use recording::{HttpExchange, HttpExchangeRequest, HttpExchangeResponse, HttpInterceptor};
pub use tool::Tool;
pub use tool_to_executor_adapter::ToolToExecutorAdapter;
