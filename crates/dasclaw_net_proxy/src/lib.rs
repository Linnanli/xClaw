//! rama-based MITM proxy with self-signed CA (codex port).
//!
//! W1 skeleton — trait surface only, no impl.
//! See `docs/plans/architecture-refactor/31-target-architecture.md` §4 for design.

#![allow(dead_code)]

/// Placeholder error type. Replaced with module-specific errors in W2+.
#[derive(Debug, thiserror::Error)]
#[error("dasclaw_net_proxy skeleton error: {0}")]
pub struct SkeletonError(pub String);

/// Primary entry trait (placeholder). Replaced with full surface in W2+.
pub trait NetProxy {
    /// rama-based MITM proxy with self-signed CA (codex port).
    fn start(&self) -> Result<(), SkeletonError>;
}

#[cfg(test)]
mod tests {
    #[test]
    fn skeleton_compiles() { /* W1 placeholder */
    }
}
