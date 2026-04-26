//! 4-stage feature flag lifecycle (codex port).
//!
//! W1 skeleton — trait surface only, no impl.
//! See `docs/plans/architecture-refactor/31-target-architecture.md` §4 for design.

#![allow(dead_code)]

/// Placeholder error type. Replaced with module-specific errors in W2+.
#[derive(Debug, thiserror::Error)]
#[error("dasclaw_features skeleton error: {0}")]
pub struct SkeletonError(pub String);


/// Primary entry trait (placeholder). Replaced with full surface in W2+.
pub trait FeatureFlags {
    /// 4-stage feature flag lifecycle (codex port).
    fn is_enabled(&self, flag: &str) -> bool;
}

#[cfg(test)]
mod tests {
    #[test]
    fn skeleton_compiles() { /* W1 placeholder */ }
}
