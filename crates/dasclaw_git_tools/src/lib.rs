//! Git operation toolset: branch/commit/diff/log/push/runner/stale/status (fork private cargo, ~1,331 LOC).
//!
//! W1 skeleton — trait surface only, no impl.
//! See `docs/plans/architecture-refactor/31-target-architecture.md` §4 for design.

#![allow(dead_code)]

/// Placeholder error type. Replaced with module-specific errors in W2+.
#[derive(Debug, thiserror::Error)]
#[error("dasclaw_git_tools skeleton error: {0}")]
pub struct SkeletonError(pub String);

/// Primary entry trait (placeholder). Replaced with full surface in W2+.
pub trait GitTool {
    /// Git operation toolset: branch/commit/diff/log/push/runner/stale/status (fork private cargo, ~1,331 LOC).
    fn run(&self, args: &[&str]) -> Result<String, SkeletonError>;
}

#[cfg(test)]
mod tests {
    #[test]
    fn skeleton_compiles() { /* W1 placeholder */
    }
}
