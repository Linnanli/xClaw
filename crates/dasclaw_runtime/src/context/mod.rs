//! Per-job context: state machine, manager, fallback delivery.
//!
//! ADR-152 §3 F3.6 slice A'' (per §11.8.9): `state`, `manager`, `fallback`
//! verbatim moved from `desktop-client/ironclaw/src/context/` into
//! `dasclaw_runtime::context`. `Memory` and `JobError` remain in
//! `dasclaw_core` and are referenced cross-crate.

pub mod fallback;
pub mod manager;
pub mod state;
mod util;

pub use fallback::FallbackDeliverable;
pub use manager::{ContextManager, ContextSummary};
pub use state::JobContext;

// verbatim 兼容 re-export：搬迁前 manager.rs 内测试使用 `crate::context::JobState`
// / `crate::context::Memory` 全路径。让旧路径仍可用。
pub use crate::JobState;
pub use dasclaw_core::context::memory::Memory;
