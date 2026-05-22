//! Routines, guardrails, and proactive execution subsystems.
//!
//! This top-level module groups the ironclaw-original capabilities that sit
//! *around* the core agent loop rather than inside it:
//!
//! - [`routine`] / [`routine_engine`]: user-owned persistent tasks with
//!   cron / event / manual triggers (lightweight single-call or full-job).
//! - [`scheduler`]: parallel job scheduling for full-job routines and
//!   autonomous worker execution.
//! - [`self_repair`]: stuck-job detection and broken-tool auto-rebuild.
//! - [`cost_guard`]: cost / rate limits per day / hour / user.
//! - [`heartbeat`]: proactive periodic agent turns driven by
//!   `HEARTBEAT.md`.
//! - [`job_monitor`]: bridge from sandbox sub-job events back into the main
//!   agent message stream.
//!
//! Previously these lived under `crate::agent::*`. The move to
//! `crate::routines::*` (Phase 3, plan H'') keeps them from cluttering the
//! agent runtime namespace while preserving the ability to extract them into
//! an independent crate later without further moves.
//!
//! The old `crate::agent::{routine, routine_engine, ...}` paths remain
//! available as re-exports in `crate::agent::mod` for backward compatibility.

pub mod cost_guard;
pub mod heartbeat;
pub mod job_monitor;
/// F4.2.0 起 `routine` 模块物理位于 `dasclaw_routines::routine`，
/// 通过 re-export 保留 `crate::routines::routine` 旧路径（ADR-129 §1.3 / ADR-152 §3 F4.2）。
pub use dasclaw_routines::routine;
pub mod routine_engine;
pub(crate) mod scheduler;
pub mod self_repair;
