//! Device-key (P256 ECDSA) + agent-identity (codex port).
//!
//! W1 skeleton — trait surface only, no impl.
//! See `docs/plans/architecture-refactor/31-target-architecture.md` §4 for design.

#![allow(dead_code)]

/// Placeholder error type. Replaced with module-specific errors in W2+.
#[derive(Debug, thiserror::Error)]
#[error("dasclaw_identity skeleton error: {0}")]
pub struct SkeletonError(pub String);


/// Primary entry trait (placeholder). Replaced with full surface in W2+.
pub trait Identity {
    /// Device-key (P256 ECDSA) + agent-identity (codex port).
    fn sign(&self, data: &[u8]) -> Result<Vec<u8>, SkeletonError>;
}

#[cfg(test)]
mod tests {
    #[test]
    fn skeleton_compiles() { /* W1 placeholder */ }
}
