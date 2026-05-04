//! Green contract — declarative test-greenness contracts (level 0–3).
//!
//! Ported from `claw-code/rust/crates/runtime/src/green_contract.rs` (W4 #70).
//!
//! A `GreenContract` declares the minimum [`GreenLevel`] required for a lane
//! to be considered "merge-green". Evaluation accepts an `Option<GreenLevel>`
//! (None = no observation yet) and returns a [`GreenContractOutcome`].
//!
//! Level ordering (`PartialOrd`/`Ord`) is **stable, ascending**:
//! `TargetedTests < Package < Workspace < MergeReady`.
//!
//! Lane-completion integration helper [`evaluate_completed_lane`] is gated
//! behind both the `green_contract` and `policy_engine` features so the
//! green contract's policy effect (CloseoutLane / CleanupSession) can be
//! exercised by integration tests when both modules are active. The
//! `AgentOutput`-shaped `detect_lane_completion` adapter from the source
//! crate is **not** ported here (depends on a `tools::AgentOutput` type that
//! doesn't exist in x-claw yet) — see Non-target in PR #70 for rationale.

use serde::{Deserialize, Serialize};

/// Green level a lane has reached. Higher = more verified.
///
/// 4 levels mapped to issue requirement "level 0–3":
/// - 0 = `TargetedTests`
/// - 1 = `Package`
/// - 2 = `Workspace`
/// - 3 = `MergeReady`
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GreenLevel {
    /// Level 0 — only the targeted tests for the change ran green.
    TargetedTests,
    /// Level 1 — the owning package's full test suite ran green.
    Package,
    /// Level 2 — the entire workspace ran green.
    Workspace,
    /// Level 3 — workspace + integration + reviewer sign-off; ready to merge.
    MergeReady,
}

impl GreenLevel {
    /// Stable string identifier used by config, audit logs, and serde.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TargetedTests => "targeted_tests",
            Self::Package => "package",
            Self::Workspace => "workspace",
            Self::MergeReady => "merge_ready",
        }
    }
}

impl std::fmt::Display for GreenLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Contract declaring the minimum [`GreenLevel`] required to merge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GreenContract {
    /// Minimum green level the lane must reach.
    pub required_level: GreenLevel,
}

impl GreenContract {
    /// Create a contract requiring at least `required_level`.
    #[must_use]
    pub fn new(required_level: GreenLevel) -> Self {
        Self { required_level }
    }

    /// Evaluate the contract against the current observation.
    ///
    /// `None` for `observed_level` means "no green report yet" — always
    /// `Unsatisfied`. Any observed level >= `required_level` satisfies.
    #[must_use]
    pub fn evaluate(self, observed_level: Option<GreenLevel>) -> GreenContractOutcome {
        match observed_level {
            Some(level) if level >= self.required_level => GreenContractOutcome::Satisfied {
                required_level: self.required_level,
                observed_level: level,
            },
            _ => GreenContractOutcome::Unsatisfied {
                required_level: self.required_level,
                observed_level,
            },
        }
    }

    /// Convenience predicate when the caller already has a definite level.
    #[must_use]
    pub fn is_satisfied_by(self, observed_level: GreenLevel) -> bool {
        observed_level >= self.required_level
    }
}

/// Outcome of evaluating a [`GreenContract`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum GreenContractOutcome {
    /// `observed_level >= required_level`.
    Satisfied {
        required_level: GreenLevel,
        observed_level: GreenLevel,
    },
    /// `observed_level < required_level` (or `None`).
    Unsatisfied {
        required_level: GreenLevel,
        observed_level: Option<GreenLevel>,
    },
}

impl GreenContractOutcome {
    /// `true` iff the contract was satisfied.
    #[must_use]
    pub fn is_satisfied(&self) -> bool {
        matches!(self, Self::Satisfied { .. })
    }
}

// ---------------------------------------------------------------------------
// Lane completion ↔ green contract integration
// ---------------------------------------------------------------------------

/// Map a [`GreenLevel`] to the `u8` level used by
/// [`crate::policy_engine::PolicyCondition::GreenAt`].
///
/// `TargetedTests = 0, Package = 1, Workspace = 2, MergeReady = 3` — matches
/// the issue title's "level 0-3" requirement and the policy_engine contract.
#[must_use]
pub fn level_as_u8(level: GreenLevel) -> u8 {
    match level {
        GreenLevel::TargetedTests => 0,
        GreenLevel::Package => 1,
        GreenLevel::Workspace => 2,
        GreenLevel::MergeReady => 3,
    }
}

/// Lane-completion → policy_engine integration helper.
///
/// Given a completed [`crate::policy_engine::LaneContext`], returns the
/// [`crate::policy_engine::PolicyAction`]s the green-contract policy fires
/// (closeout + cleanup). Gated on `policy_engine` because it consumes the
/// engine's types directly.
#[cfg(feature = "policy_engine")]
#[must_use]
pub fn evaluate_completed_lane(
    context: &crate::policy_engine::LaneContext,
) -> Vec<crate::policy_engine::PolicyAction> {
    use crate::policy_engine::{evaluate, PolicyAction, PolicyCondition, PolicyEngine, PolicyRule};

    let engine = PolicyEngine::new(vec![
        PolicyRule::new(
            "closeout-completed-lane",
            PolicyCondition::And(vec![
                PolicyCondition::LaneCompleted,
                PolicyCondition::GreenAt {
                    level: level_as_u8(GreenLevel::MergeReady),
                },
            ]),
            PolicyAction::CloseoutLane,
            10,
        ),
        PolicyRule::new(
            "cleanup-completed-session",
            PolicyCondition::LaneCompleted,
            PolicyAction::CleanupSession,
            5,
        ),
    ]);

    evaluate(&engine, context)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_green_contract_70_matching_level_satisfies_contract() {
        let contract = GreenContract::new(GreenLevel::Package);

        let outcome = contract.evaluate(Some(GreenLevel::Package));

        assert_eq!(
            outcome,
            GreenContractOutcome::Satisfied {
                required_level: GreenLevel::Package,
                observed_level: GreenLevel::Package,
            }
        );
        assert!(outcome.is_satisfied());
    }

    #[test]
    fn req_green_contract_70_higher_level_still_satisfies() {
        let contract = GreenContract::new(GreenLevel::TargetedTests);

        assert!(contract.is_satisfied_by(GreenLevel::Workspace));
        assert!(contract.is_satisfied_by(GreenLevel::MergeReady));
    }

    #[test]
    fn req_green_contract_70_lower_level_unsatisfied() {
        let contract = GreenContract::new(GreenLevel::Workspace);

        let outcome = contract.evaluate(Some(GreenLevel::Package));

        assert_eq!(
            outcome,
            GreenContractOutcome::Unsatisfied {
                required_level: GreenLevel::Workspace,
                observed_level: Some(GreenLevel::Package),
            }
        );
        assert!(!outcome.is_satisfied());
    }

    #[test]
    fn req_green_contract_70_no_observation_unsatisfied() {
        let contract = GreenContract::new(GreenLevel::MergeReady);

        let outcome = contract.evaluate(None);

        assert_eq!(
            outcome,
            GreenContractOutcome::Unsatisfied {
                required_level: GreenLevel::MergeReady,
                observed_level: None,
            }
        );
        assert!(!outcome.is_satisfied());
    }

    #[test]
    fn req_green_contract_70_level_ordering_is_ascending() {
        assert!(GreenLevel::TargetedTests < GreenLevel::Package);
        assert!(GreenLevel::Package < GreenLevel::Workspace);
        assert!(GreenLevel::Workspace < GreenLevel::MergeReady);
    }

    #[test]
    fn req_green_contract_70_level_as_u8_maps_zero_through_three() {
        assert_eq!(level_as_u8(GreenLevel::TargetedTests), 0);
        assert_eq!(level_as_u8(GreenLevel::Package), 1);
        assert_eq!(level_as_u8(GreenLevel::Workspace), 2);
        assert_eq!(level_as_u8(GreenLevel::MergeReady), 3);
    }

    #[test]
    fn req_green_contract_70_as_str_round_trip() {
        for level in [
            GreenLevel::TargetedTests,
            GreenLevel::Package,
            GreenLevel::Workspace,
            GreenLevel::MergeReady,
        ] {
            assert_eq!(level.to_string(), level.as_str());
        }
    }

    #[cfg(feature = "policy_engine")]
    #[test]
    fn req_green_contract_70_evaluate_completed_lane_fires_closeout_and_cleanup() {
        use crate::policy_engine::{
            DiffScope, LaneBlocker, LaneContext, PolicyAction, ReviewStatus,
        };
        use std::time::Duration;

        let ctx = LaneContext {
            lane_id: "completed-lane".to_string(),
            green_level: level_as_u8(GreenLevel::MergeReady),
            branch_freshness: Duration::from_secs(0),
            blocker: LaneBlocker::None,
            review_status: ReviewStatus::Approved,
            diff_scope: DiffScope::Scoped,
            completed: true,
            reconciled: false,
        };

        let actions = evaluate_completed_lane(&ctx);

        assert!(
            actions.contains(&PolicyAction::CloseoutLane),
            "expected CloseoutLane in {actions:?}"
        );
        assert!(
            actions.contains(&PolicyAction::CleanupSession),
            "expected CleanupSession in {actions:?}"
        );
    }

    #[cfg(feature = "policy_engine")]
    #[test]
    fn req_green_contract_70_evaluate_incomplete_lane_no_closeout() {
        use crate::policy_engine::{
            DiffScope, LaneBlocker, LaneContext, PolicyAction, ReviewStatus,
        };
        use std::time::Duration;

        // A lane that has NOT completed — closeout/cleanup must NOT fire.
        let ctx = LaneContext {
            lane_id: "active-lane".to_string(),
            green_level: level_as_u8(GreenLevel::MergeReady),
            branch_freshness: Duration::from_secs(0),
            blocker: LaneBlocker::None,
            review_status: ReviewStatus::Approved,
            diff_scope: DiffScope::Scoped,
            completed: false,
            reconciled: false,
        };

        let actions = evaluate_completed_lane(&ctx);

        assert!(!actions.contains(&PolicyAction::CloseoutLane));
        assert!(!actions.contains(&PolicyAction::CleanupSession));
    }
}
