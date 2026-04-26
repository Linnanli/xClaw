//! Unified observability + rollout-trace 4 sub-tables (codex + ironclaw).
//!
//! W1 skeleton — trait surface only, no impl.
//! See `docs/plans/architecture-refactor/31-target-architecture.md` §4 for design.

#![allow(dead_code)]

/// Placeholder error type. Replaced with module-specific errors in W2+.
#[derive(Debug, thiserror::Error)]
#[error("dasclaw_observability skeleton error: {0}")]
pub struct SkeletonError(pub String);

pub struct ObservabilityEvent;

/// Primary entry trait (placeholder). Replaced with full surface in W2+.
pub trait Observer {
    /// Unified observability + rollout-trace 4 sub-tables (codex + ironclaw).
    fn record(&self, event: ObservabilityEvent);
}

#[cfg(test)]
mod tests {
    #[test]
    fn skeleton_compiles() { /* W1 placeholder */ }
}
