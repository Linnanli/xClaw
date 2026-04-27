//! Git tools for version control operations.
//!
//! Provides typed tools that shell out to `git` CLI with risk-level classification:
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
