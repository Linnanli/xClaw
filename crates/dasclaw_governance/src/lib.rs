//! Self-governance 6-pack: policy / recovery / trust / branch_lock / stale / green.
//!
//! W1 skeleton — trait surface only, no impl.
//! See `docs/plans/architecture-refactor/31-target-architecture.md` §4 for design.

#![allow(dead_code)]

/// Placeholder error type. Replaced with module-specific errors in W2+.
#[derive(Debug, thiserror::Error)]
#[error("dasclaw_governance skeleton error: {0}")]
pub struct SkeletonError(pub String);

pub struct GovernanceContext;
pub enum GovernanceDecision { Allow, Deny, Escalate }

/// Primary entry trait (placeholder). Replaced with full surface in W2+.
pub trait GovernanceEngine {
    /// Self-governance 6-pack: policy / recovery / trust / branch_lock / stale / green.
    fn evaluate(&self, ctx: &GovernanceContext) -> GovernanceDecision;
}

#[cfg(test)]
mod tests {
    #[test]
    fn skeleton_compiles() { /* W1 placeholder */ }
}
