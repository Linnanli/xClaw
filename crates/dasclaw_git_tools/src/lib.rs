//! F4.6.5 — Git builtin tools extracted from
//! `desktop-client/ironclaw/src/tools/builtin/git/` per
//! [ADR-156 §6.3](../../../docs/plans/architecture-refactor/adr-156-f46-builtin-tools-landing-decision.md).
//!
//! Replaces the W1 skeleton (`SkeletonError` + placeholder `GitTool` trait)
//! with the real implementation: shells out to `git` CLI via a shared runner
//! with timeout, output capture, and risk-level classification.
//!
//! Verbatim port (ADR-129 §1.3): file bodies are byte-for-byte copies of the
//! desktop sources, with only `use crate::tools::tool::{...}` rewritten to
//! `dasclaw_runtime::Tool` + `dasclaw_tool::{...}` following the workspace
//! dependency direction (ADR-154 §3.1 amendment).
//!
//! | Tool            | Risk   | Approval          |
//! |-----------------|--------|-------------------|
//! | git_status      | Low    | Never             |
//! | git_diff        | Low    | Never             |
//! | git_log         | Low    | Never             |
//! | git_branch      | Medium | UnlessAutoApproved|
//! | git_commit      | Medium | UnlessAutoApproved|
//! | git_push        | High   | Always            |
//! | git_stale_check | Low    | Never             |

mod branch;
mod commit;
mod diff;
mod log;
mod push;
mod runner;
mod stale;
mod status;

pub use branch::GitBranchTool;
pub use commit::GitCommitTool;
pub use diff::GitDiffTool;
pub use log::GitLogTool;
pub use push::GitPushTool;
pub use stale::GitStaleCheckTool;
pub use status::GitStatusTool;
