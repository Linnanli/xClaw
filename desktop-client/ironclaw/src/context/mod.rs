//! Per-job context isolation and state management.
//!
//! Each job runs with its own isolated context that includes:
//! - Conversation history
//! - Action history
//! - State machine
//! - Resource tracking

pub mod fallback;
mod manager;
mod state;

pub use fallback::FallbackDeliverable;
pub use manager::ContextManager;
// ADR-152 §3 F3.6 slice 1: `memory` was moved into `dasclaw_core::context::memory`.
// Re-export keeps existing `crate::context::{ActionRecord, ConversationMemory, Memory}`
// call sites (e.g. `worker::job`) source-compatible.
pub use dasclaw_core::context::memory::{ActionRecord, ConversationMemory, Memory};
pub use state::{JobContext, JobState, StateTransition, TokenBudgetExceeded};
