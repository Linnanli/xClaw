//! `x_claw_agent` — agent runtime fork of `claw-code` for the x-claw project.
//!
//! This crate starts as an **empty scaffold** (Phase 3 Step B). Subsequent steps
//! will:
//!
//! - Step C: define Hook traits (`SafetyHook`, `SandboxExecutor`, `SecretProvider`,
//!   `ApprovalGate`) in [`hooks`].
//! - Step D: port the runtime loop from `claw-code`'s runtime crate into
//!   [`runtime`] with hook insertion points.
//!
//! See [`../UPSTREAM_BASELINE.md`](../UPSTREAM_BASELINE.md) for the chosen
//! upstream commit and porting log.

/// Crate version string, exposed so downstream crates can surface the baseline
/// to operators without reparsing `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_has_version() {
        assert!(!VERSION.is_empty());
    }
}
