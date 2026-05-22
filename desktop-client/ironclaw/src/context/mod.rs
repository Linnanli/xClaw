//! Per-job context: re-export shim.
//!
//! ADR-152 §3 F3.6 (slice 1 / slice 2 / slice A''): `Memory` + `JobError`
//! live in `dasclaw_core`; `state` + `manager` + `fallback` live in
//! `dasclaw_runtime::context`. This shim keeps existing `crate::context::*`
//! call sites source-compatible.

pub use dasclaw_core::context::memory::{ActionRecord, ConversationMemory, Memory};
pub use dasclaw_runtime::context::{
    ContextManager, ContextSummary, FallbackDeliverable, JobContext,
};
pub use dasclaw_runtime::{JobState, StateTransition, TokenBudgetExceeded};
