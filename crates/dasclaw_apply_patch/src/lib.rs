//! Lark-grammar apply_patch protocol (codex port).
//!
//! W1 skeleton — trait surface only, no impl.
//! See `docs/plans/architecture-refactor/31-target-architecture.md` §4 for design.

#![allow(dead_code)]

/// Placeholder error type. Replaced with module-specific errors in W2+.
#[derive(Debug, thiserror::Error)]
#[error("dasclaw_apply_patch skeleton error: {0}")]
pub struct SkeletonError(pub String);


/// Primary entry trait (placeholder). Replaced with full surface in W2+.
pub trait ApplyPatch {
    /// Lark-grammar apply_patch protocol (codex port).
    fn apply(&self, patch: &str) -> Result<(), SkeletonError>;
}

#[cfg(test)]
mod tests {
    #[test]
    fn skeleton_compiles() { /* W1 placeholder */ }
}
