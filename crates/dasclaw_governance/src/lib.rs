//! Self-governance 6-pack: policy / recovery / trust / branch_lock / stale / green.
//!
//! W4 scaffolding — module placeholders behind cargo features. Implementations
//! land in follow-up issues:
//!
//! | Module             | Issue | Cargo feature       |
//! |--------------------|-------|---------------------|
//! | `policy_engine`    | #65   | `policy_engine`     |
//! | `recovery_recipes` | #66   | `recovery_recipes`  |
//! | `trust_resolver`   | #67   | `trust_resolver`    |
//! | `branch_lock`      | #68   | `branch_lock`       |
//! | `stale_base`       | #69   | `stale_base`        |
//! | `stale_branch`     | #69   | `stale_branch`      |
//! | `green_contract`   | #70   | `green_contract`    |
//! | `lane_events`      | #71   | `lane_events`       |
//!
//! All features default to **off**. See `docs/plans/architecture-refactor/
//! 31-target-architecture.md` §4 and `32-execution-plan.md` W4 for design.

#![allow(dead_code)]

/// Placeholder error type. Replaced with module-specific errors as each
/// 6-pack module lands its real implementation.
#[derive(Debug, thiserror::Error)]
#[error("dasclaw_governance skeleton error: {0}")]
pub struct SkeletonError(pub String);

pub struct GovernanceContext;

pub enum GovernanceDecision {
    Allow,
    Deny,
    Escalate,
}

/// Primary entry trait (placeholder). Replaced with the full surface once
/// `policy_engine` (#65) lands.
pub trait GovernanceEngine {
    /// Self-governance 6-pack: policy / recovery / trust / branch_lock / stale / green.
    fn evaluate(&self, ctx: &GovernanceContext) -> GovernanceDecision;
}

// ---- 6-pack module placeholders (feature-gated, default OFF) ----------------

#[cfg(feature = "policy_engine")]
pub mod policy_engine;

#[cfg(feature = "recovery_recipes")]
pub mod recovery_recipes;

#[cfg(feature = "trust_resolver")]
pub mod trust_resolver;

#[cfg(feature = "branch_lock")]
pub mod branch_lock;

#[cfg(feature = "stale_base")]
pub mod stale_base;

#[cfg(feature = "stale_branch")]
pub mod stale_branch;

#[cfg(feature = "green_contract")]
pub mod green_contract;

#[cfg(feature = "lane_events")]
pub mod lane_events;

#[cfg(feature = "team_cron_registry")]
pub mod team_cron_registry;

#[cfg(feature = "task_packet")]
pub mod task_packet;

#[cfg(feature = "task_registry")]
pub mod task_registry;

#[cfg(feature = "tool_visibility")]
pub mod tool_visibility;

// ---- ADR-148 EgressGate: always-on Layer B trait seam ----------------------
//
// Unlike the feature-gated W4 6-pack scaffolds above, [`egress`] is core
// infrastructure consumed by `dasclaw_hooks` / `dasclaw_core` /
// `ironclaw_safety` and is therefore unconditional. See
// `docs/plans/architecture-refactor/adr-148-egress-gate-safety-hook-semantics.md`.
pub mod egress;

pub use egress::{
    CompositeEgressGate, CompositeEgressGateBuilder, EgressDecision, EgressGate, EgressKind,
    GateId, NoopEgressGate, RedactionStats, RuleAction, RuleSuggestion,
};

#[cfg(test)]
mod tests {
    #[test]
    fn skeleton_compiles() { /* W4 placeholder */
    }

    /// `req_governance_64_default_features_off` — runtime check that the
    /// crate's default `Cargo.toml` features list is empty. Reads the
    /// manifest at test time; fails if someone adds anything to
    /// `[features] default = [...]`. Guards W4 contract: scaffolding only.
    #[test]
    fn req_governance_64_default_features_off() {
        let manifest = include_str!("../Cargo.toml");
        // Look for the `default = ...` line inside the [features] block.
        let features_block = manifest
            .split("[features]")
            .nth(1)
            .expect("Cargo.toml must contain [features] block");
        let default_line = features_block
            .lines()
            .find(|l| l.trim_start().starts_with("default"))
            .expect("[features] must declare default = []");
        assert!(
            default_line.contains("[]"),
            "default features must be empty (W4 contract); got: {default_line}"
        );
    }
}
