//! `policy_engine` — declarative rule engine for lane decisions.
//!
//! Ported from `claw-code/rust/crates/runtime/src/policy_engine.rs`.
//!
//! # Surface
//!
//! - [`PolicyCondition`] — leaves (`GreenAt`, `StaleBranch`, `LaneCompleted`, …)
//!   plus `And` / `Or` combinators.
//! - [`PolicyAction`] — primitives (`MergeToDev`, `RecoverOnce`, `Escalate`, …)
//!   plus the `Chain` recursive flattening combinator.
//! - [`LaneContext`] — pure data struct describing the lane snapshot used for
//!   evaluation.
//! - [`PolicyRule`] — `(name, condition, action, priority)` 4-tuple.
//! - [`PolicyEngine::evaluate`] / [`evaluate`] — sort by priority (stable), match,
//!   flatten chains, return action list.
//!
//! Engine is **read-only** with respect to the lane context: callers stay
//! responsible for executing actions. This keeps `evaluate` deterministic and
//! cheap (pure function over inputs).

use std::time::Duration;

/// Green-tier counter (0 = red, higher = greener). Matches claw-code semantics.
pub type GreenLevel = u8;

/// Threshold beyond which `PolicyCondition::StaleBranch` fires.
///
/// Mirrors the claw-code default; callers needing a different threshold can use
/// `PolicyCondition::TimedOut { duration }` instead.
pub const STALE_BRANCH_THRESHOLD: Duration = Duration::from_secs(60 * 60);

/// A single named rule: 当 `condition` 命中时产出 `action`。`priority` 决定排序
/// （越小越先），同优先级保持插入顺序（stable sort）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyRule {
    pub name: String,
    pub condition: PolicyCondition,
    pub action: PolicyAction,
    pub priority: u32,
}

impl PolicyRule {
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        condition: PolicyCondition,
        action: PolicyAction,
        priority: u32,
    ) -> Self {
        Self {
            name: name.into(),
            condition,
            action,
            priority,
        }
    }

    #[must_use]
    pub fn matches(&self, context: &LaneContext) -> bool {
        self.condition.matches(context)
    }
}

/// Boolean tree describing when a rule fires.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyCondition {
    /// Vacuous-true on empty: empty `And` matches every context.
    And(Vec<PolicyCondition>),
    /// Vacuous-false on empty: empty `Or` never matches.
    Or(Vec<PolicyCondition>),
    GreenAt {
        level: GreenLevel,
    },
    StaleBranch,
    StartupBlocked,
    LaneCompleted,
    LaneReconciled,
    ReviewPassed,
    ScopedDiff,
    TimedOut {
        duration: Duration,
    },
}

impl PolicyCondition {
    #[must_use]
    pub fn matches(&self, context: &LaneContext) -> bool {
        match self {
            Self::And(conditions) => conditions.iter().all(|c| c.matches(context)),
            Self::Or(conditions) => conditions.iter().any(|c| c.matches(context)),
            Self::GreenAt { level } => context.green_level >= *level,
            Self::StaleBranch => context.branch_freshness >= STALE_BRANCH_THRESHOLD,
            Self::StartupBlocked => context.blocker == LaneBlocker::Startup,
            Self::LaneCompleted => context.completed,
            Self::LaneReconciled => context.reconciled,
            Self::ReviewPassed => context.review_status == ReviewStatus::Approved,
            Self::ScopedDiff => context.diff_scope == DiffScope::Scoped,
            Self::TimedOut { duration } => context.branch_freshness >= *duration,
        }
    }
}

/// Action emitted by a matched rule. `Chain` is flattened recursively at
/// evaluation time so the caller receives a flat action list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyAction {
    MergeToDev,
    MergeForward,
    RecoverOnce,
    Escalate { reason: String },
    CloseoutLane,
    CleanupSession,
    Reconcile { reason: ReconcileReason },
    Notify { channel: String },
    Block { reason: String },
    Chain(Vec<PolicyAction>),
}

/// Why a lane was reconciled without further action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconcileReason {
    /// Branch already merged into main — no PR needed.
    AlreadyMerged,
    /// Work superseded by another lane or direct commit.
    Superseded,
    /// PR would be empty — all changes already landed.
    EmptyDiff,
    /// Lane manually closed by operator.
    ManualClose,
}

impl PolicyAction {
    fn flatten_into(&self, actions: &mut Vec<PolicyAction>) {
        match self {
            Self::Chain(chained) => {
                for action in chained {
                    action.flatten_into(actions);
                }
            }
            other => actions.push(other.clone()),
        }
    }
}

/// What is currently blocking the lane (or `None`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaneBlocker {
    None,
    Startup,
    External,
}

/// Reviewer state on the lane's PR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewStatus {
    Pending,
    Approved,
    Rejected,
}

/// Whether the lane diff stayed in scope (`Scoped`) or sprawled (`Full`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffScope {
    Full,
    Scoped,
}

/// Lane snapshot the engine evaluates. Pure data — no I/O happens during
/// matching.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaneContext {
    pub lane_id: String,
    pub green_level: GreenLevel,
    pub branch_freshness: Duration,
    pub blocker: LaneBlocker,
    pub review_status: ReviewStatus,
    pub diff_scope: DiffScope,
    pub completed: bool,
    pub reconciled: bool,
}

impl LaneContext {
    #[must_use]
    pub fn new(
        lane_id: impl Into<String>,
        green_level: GreenLevel,
        branch_freshness: Duration,
        blocker: LaneBlocker,
        review_status: ReviewStatus,
        diff_scope: DiffScope,
        completed: bool,
    ) -> Self {
        Self {
            lane_id: lane_id.into(),
            green_level,
            branch_freshness,
            blocker,
            review_status,
            diff_scope,
            completed,
            reconciled: false,
        }
    }

    /// Lane that has already been reconciled (no further action needed).
    #[must_use]
    pub fn reconciled(lane_id: impl Into<String>) -> Self {
        Self {
            lane_id: lane_id.into(),
            green_level: 0,
            branch_freshness: Duration::from_secs(0),
            blocker: LaneBlocker::None,
            review_status: ReviewStatus::Pending,
            diff_scope: DiffScope::Full,
            completed: true,
            reconciled: true,
        }
    }
}

/// Compiled rule set. Rules are stored sorted by priority (stable on ties) so
/// `evaluate` is just a linear scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyEngine {
    rules: Vec<PolicyRule>,
}

impl PolicyEngine {
    #[must_use]
    pub fn new(mut rules: Vec<PolicyRule>) -> Self {
        // sort_by_key is stable in std (sort uses Timsort), preserving
        // insertion order on equal priorities — required by the priority
        // ordering test.
        rules.sort_by_key(|rule| rule.priority);
        Self { rules }
    }

    #[must_use]
    pub fn rules(&self) -> &[PolicyRule] {
        &self.rules
    }

    #[must_use]
    pub fn evaluate(&self, context: &LaneContext) -> Vec<PolicyAction> {
        evaluate(self, context)
    }
}

/// Free-function form of [`PolicyEngine::evaluate`]. Returns the flattened
/// action list of every rule whose condition matched, in priority order.
#[must_use]
pub fn evaluate(engine: &PolicyEngine, context: &LaneContext) -> Vec<PolicyAction> {
    let mut actions = Vec::new();
    for rule in &engine.rules {
        if rule.matches(context) {
            rule.action.flatten_into(&mut actions);
        }
    }
    actions
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{
        evaluate, DiffScope, LaneBlocker, LaneContext, PolicyAction, PolicyCondition, PolicyEngine,
        PolicyRule, ReconcileReason, ReviewStatus, STALE_BRANCH_THRESHOLD,
    };

    fn default_context() -> LaneContext {
        LaneContext::new(
            "lane-7",
            0,
            Duration::from_secs(0),
            LaneBlocker::None,
            ReviewStatus::Pending,
            DiffScope::Full,
            false,
        )
    }

    #[test]
    fn req_policy_engine_65_merge_to_dev_fires_for_green_scoped_reviewed_lane() {
        let engine = PolicyEngine::new(vec![PolicyRule::new(
            "merge-to-dev",
            PolicyCondition::And(vec![
                PolicyCondition::GreenAt { level: 2 },
                PolicyCondition::ScopedDiff,
                PolicyCondition::ReviewPassed,
            ]),
            PolicyAction::MergeToDev,
            20,
        )]);
        let context = LaneContext::new(
            "lane-7",
            3,
            Duration::from_secs(5),
            LaneBlocker::None,
            ReviewStatus::Approved,
            DiffScope::Scoped,
            false,
        );

        assert_eq!(engine.evaluate(&context), vec![PolicyAction::MergeToDev]);
    }

    #[test]
    fn req_policy_engine_65_stale_branch_fires_at_threshold() {
        let engine = PolicyEngine::new(vec![PolicyRule::new(
            "merge-forward",
            PolicyCondition::StaleBranch,
            PolicyAction::MergeForward,
            10,
        )]);
        let context = LaneContext::new(
            "lane-7",
            1,
            STALE_BRANCH_THRESHOLD,
            LaneBlocker::None,
            ReviewStatus::Pending,
            DiffScope::Full,
            false,
        );

        assert_eq!(engine.evaluate(&context), vec![PolicyAction::MergeForward]);
    }

    #[test]
    fn req_policy_engine_65_startup_blocked_recovers_then_escalates() {
        let engine = PolicyEngine::new(vec![PolicyRule::new(
            "startup-recovery",
            PolicyCondition::StartupBlocked,
            PolicyAction::Chain(vec![
                PolicyAction::RecoverOnce,
                PolicyAction::Escalate {
                    reason: "startup remained blocked".to_string(),
                },
            ]),
            15,
        )]);
        let context = LaneContext::new(
            "lane-7",
            0,
            Duration::from_secs(0),
            LaneBlocker::Startup,
            ReviewStatus::Pending,
            DiffScope::Full,
            false,
        );

        assert_eq!(
            engine.evaluate(&context),
            vec![
                PolicyAction::RecoverOnce,
                PolicyAction::Escalate {
                    reason: "startup remained blocked".to_string(),
                },
            ]
        );
    }

    #[test]
    fn req_policy_engine_65_completed_lane_closes_out_and_cleans_up() {
        let engine = PolicyEngine::new(vec![PolicyRule::new(
            "lane-closeout",
            PolicyCondition::LaneCompleted,
            PolicyAction::Chain(vec![
                PolicyAction::CloseoutLane,
                PolicyAction::CleanupSession,
            ]),
            30,
        )]);
        let context = LaneContext::new(
            "lane-7",
            0,
            Duration::from_secs(0),
            LaneBlocker::None,
            ReviewStatus::Pending,
            DiffScope::Full,
            true,
        );

        assert_eq!(
            engine.evaluate(&context),
            vec![PolicyAction::CloseoutLane, PolicyAction::CleanupSession]
        );
    }

    #[test]
    fn req_policy_engine_65_priority_order_is_stable_on_ties() {
        let engine = PolicyEngine::new(vec![
            PolicyRule::new(
                "late-cleanup",
                PolicyCondition::And(vec![]),
                PolicyAction::CleanupSession,
                30,
            ),
            PolicyRule::new(
                "first-notify",
                PolicyCondition::And(vec![]),
                PolicyAction::Notify {
                    channel: "ops".to_string(),
                },
                10,
            ),
            PolicyRule::new(
                "second-notify",
                PolicyCondition::And(vec![]),
                PolicyAction::Notify {
                    channel: "review".to_string(),
                },
                10,
            ),
            PolicyRule::new(
                "merge",
                PolicyCondition::And(vec![]),
                PolicyAction::MergeToDev,
                20,
            ),
        ]);

        assert_eq!(
            evaluate(&engine, &default_context()),
            vec![
                PolicyAction::Notify {
                    channel: "ops".to_string(),
                },
                PolicyAction::Notify {
                    channel: "review".to_string(),
                },
                PolicyAction::MergeToDev,
                PolicyAction::CleanupSession,
            ]
        );
    }

    #[test]
    fn req_policy_engine_65_combinators_handle_empty_and_nested_chains() {
        let engine = PolicyEngine::new(vec![
            PolicyRule::new(
                "empty-and",
                PolicyCondition::And(vec![]),
                PolicyAction::Notify {
                    channel: "orchestrator".to_string(),
                },
                5,
            ),
            PolicyRule::new(
                "empty-or",
                PolicyCondition::Or(vec![]),
                PolicyAction::Block {
                    reason: "should not fire".to_string(),
                },
                10,
            ),
            PolicyRule::new(
                "nested",
                PolicyCondition::Or(vec![
                    PolicyCondition::StartupBlocked,
                    PolicyCondition::And(vec![
                        PolicyCondition::GreenAt { level: 2 },
                        PolicyCondition::TimedOut {
                            duration: Duration::from_secs(5),
                        },
                    ]),
                ]),
                PolicyAction::Chain(vec![
                    PolicyAction::Notify {
                        channel: "alerts".to_string(),
                    },
                    PolicyAction::Chain(vec![
                        PolicyAction::MergeForward,
                        PolicyAction::CleanupSession,
                    ]),
                ]),
                15,
            ),
        ]);
        let context = LaneContext::new(
            "lane-7",
            2,
            Duration::from_secs(10),
            LaneBlocker::External,
            ReviewStatus::Pending,
            DiffScope::Full,
            false,
        );

        assert_eq!(
            engine.evaluate(&context),
            vec![
                PolicyAction::Notify {
                    channel: "orchestrator".to_string(),
                },
                PolicyAction::Notify {
                    channel: "alerts".to_string(),
                },
                PolicyAction::MergeForward,
                PolicyAction::CleanupSession,
            ]
        );
    }

    #[test]
    fn req_policy_engine_65_reconciled_lane_emits_reconcile_and_cleanup() {
        let engine = PolicyEngine::new(vec![
            PolicyRule::new(
                "reconcile-closeout",
                PolicyCondition::LaneReconciled,
                PolicyAction::Chain(vec![
                    PolicyAction::Reconcile {
                        reason: ReconcileReason::AlreadyMerged,
                    },
                    PolicyAction::CloseoutLane,
                    PolicyAction::CleanupSession,
                ]),
                5,
            ),
            PolicyRule::new(
                "generic-closeout",
                PolicyCondition::And(vec![
                    PolicyCondition::LaneCompleted,
                    PolicyCondition::And(vec![]),
                ]),
                PolicyAction::CloseoutLane,
                30,
            ),
        ]);

        assert_eq!(
            engine.evaluate(&LaneContext::reconciled("lane-9411")),
            vec![
                PolicyAction::Reconcile {
                    reason: ReconcileReason::AlreadyMerged,
                },
                PolicyAction::CloseoutLane,
                PolicyAction::CleanupSession,
                PolicyAction::CloseoutLane,
            ]
        );
    }

    #[test]
    fn req_policy_engine_65_reconciled_context_has_correct_defaults() {
        let ctx = LaneContext::reconciled("test-lane");
        assert_eq!(ctx.lane_id, "test-lane");
        assert!(ctx.completed);
        assert!(ctx.reconciled);
        assert_eq!(ctx.blocker, LaneBlocker::None);
        assert_eq!(ctx.green_level, 0);
    }

    #[test]
    fn req_policy_engine_65_non_reconciled_lane_does_not_trigger_reconcile_rule() {
        let engine = PolicyEngine::new(vec![PolicyRule::new(
            "reconcile-closeout",
            PolicyCondition::LaneReconciled,
            PolicyAction::Reconcile {
                reason: ReconcileReason::EmptyDiff,
            },
            5,
        )]);
        let context = LaneContext::new(
            "lane-7",
            0,
            Duration::from_secs(0),
            LaneBlocker::None,
            ReviewStatus::Pending,
            DiffScope::Full,
            true,
        );

        assert!(engine.evaluate(&context).is_empty());
    }

    #[test]
    fn req_policy_engine_65_reconcile_reason_variants_are_distinct() {
        assert_ne!(ReconcileReason::AlreadyMerged, ReconcileReason::Superseded);
        assert_ne!(ReconcileReason::EmptyDiff, ReconcileReason::ManualClose);
    }
}
