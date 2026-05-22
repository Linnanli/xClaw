//! Routine orchestration (fork ironclaw 0.24 routines module promotion).
//!
//! F4.2.0: `routine.rs` 与 `RoutineError` 已从 `desktop-client/ironclaw`
//! 按 verbatim 方式搬迁过来（ADR-129 §1.3 / ADR-152 §3 F4.2 / §11.8.9.3）。
//!
//! routines 子系统其余模块（`routine_engine` / `scheduler` / `cost_guard` /
//! `heartbeat` / `job_monitor` / `self_repair`）仍引用大量尚未 crate 化的 desktop
//! 模块（详见 `docs/plans/architecture-refactor/f42-preflight-dependency-assessment.md`），
//! 留待后续切片。

pub mod error;
pub mod routine;

pub use error::RoutineError;
