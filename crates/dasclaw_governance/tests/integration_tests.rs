#![allow(clippy::doc_markdown, clippy::uninlined_format_args)]
//! Cross-module wiring integration tests for the governance 6-pack.
//!
//! Ported from `claw-code/rust/crates/runtime/tests/integration_tests.rs`
//! per #75 (W4 acceptance: claw-code test suite full port).
//!
//! These tests verify that adjacent modules in `dasclaw_governance` connect
//! correctly — catching wiring gaps that per-module unit tests miss.
//!
//! Invariants preserved from upstream:
//! - PolicyEngine + LaneContext + StaleBranch flow
//! - GreenContract level ordering across PolicyCondition::GreenAt
//! - LaneContext::reconciled() reconcile-before-closeout precedence
//! - WorkerFailureKind → FailureScenario → recovery → post-recovery policy
//!
//! Upstream variant `worker_provider_failure_flows_through_recovery_to_policy`
//! sourced `WorkerFailureKind`/`WorkerRegistry`/`WorkerStatus` from the
//! runtime-only `worker_boot` module. In dasclaw_governance the kind enum
//! lives in `recovery_recipes`, and `WorkerRegistry` plumbing is out of
//! crate scope. The ported test enters the recovery path directly with
//! `WorkerFailureKind::Provider`, which preserves the failure→recovery→
//! policy wiring contract under test.

use std::time::Duration;

use dasclaw_governance::green_contract::{GreenContract, GreenLevel};
use dasclaw_governance::policy_engine::{
    DiffScope, LaneBlocker, LaneContext, PolicyAction, PolicyCondition, PolicyEngine, PolicyRule,
    ReconcileReason, ReviewStatus,
};
use dasclaw_governance::recovery_recipes::{
    attempt_recovery, FailureScenario, RecoveryContext, RecoveryEvent, RecoveryResult,
    WorkerFailureKind,
};
use dasclaw_governance::stale_branch::{
    apply_policy, BranchFreshness, StaleBranchAction, StaleBranchPolicy,
};

/// stale_branch + policy_engine integration:
/// When a branch is detected stale, does it correctly flow through
/// PolicyCondition::StaleBranch to generate the expected action?
#[test]
fn stale_branch_detection_flows_into_policy_engine() {
    let stale_context = LaneContext::new(
        "stale-lane",
        0,
        Duration::from_secs(2 * 60 * 60),
        LaneBlocker::None,
        ReviewStatus::Pending,
        DiffScope::Full,
        false,
    );

    let engine = PolicyEngine::new(vec![PolicyRule::new(
        "stale-merge-forward",
        PolicyCondition::StaleBranch,
        PolicyAction::MergeForward,
        10,
    )]);

    let actions = engine.evaluate(&stale_context);
    assert_eq!(actions, vec![PolicyAction::MergeForward]);
}

/// stale_branch + policy_engine: Fresh branch does NOT trigger stale rules.
#[test]
fn fresh_branch_does_not_trigger_stale_policy() {
    let fresh_context = LaneContext::new(
        "fresh-lane",
        0,
        Duration::from_secs(30 * 60),
        LaneBlocker::None,
        ReviewStatus::Pending,
        DiffScope::Full,
        false,
    );

    let engine = PolicyEngine::new(vec![PolicyRule::new(
        "stale-merge-forward",
        PolicyCondition::StaleBranch,
        PolicyAction::MergeForward,
        10,
    )]);

    let actions = engine.evaluate(&fresh_context);
    assert!(actions.is_empty());
}

/// green_contract: lane that meets contract is satisfied; lane below is not.
#[test]
fn green_contract_satisfied_allows_merge() {
    let contract = GreenContract::new(GreenLevel::Workspace);
    assert!(contract.is_satisfied_by(GreenLevel::Workspace));
    assert!(contract.is_satisfied_by(GreenLevel::MergeReady));
    assert!(!contract.is_satisfied_by(GreenLevel::Package));
}

/// green_contract + policy_engine: lane below required green level is blocked.
#[test]
fn green_contract_unsatisfied_blocks_merge() {
    let context = LaneContext::new(
        "partial-green-lane",
        1,
        Duration::from_secs(0),
        LaneBlocker::None,
        ReviewStatus::Pending,
        DiffScope::Full,
        false,
    );

    let engine = PolicyEngine::new(vec![PolicyRule::new(
        "workspace-green-required",
        PolicyCondition::GreenAt { level: 3 },
        PolicyAction::MergeToDev,
        10,
    )]);

    let actions = engine.evaluate(&context);
    assert!(actions.is_empty());
}

/// reconciliation + policy_engine: reconcile rule fires before generic closeout.
#[test]
fn reconciled_lane_matches_reconcile_condition() {
    let context = LaneContext::reconciled("reconciled-lane");

    let engine = PolicyEngine::new(vec![
        PolicyRule::new(
            "reconcile-first",
            PolicyCondition::LaneReconciled,
            PolicyAction::Reconcile {
                reason: ReconcileReason::AlreadyMerged,
            },
            5,
        ),
        PolicyRule::new(
            "generic-closeout",
            PolicyCondition::LaneCompleted,
            PolicyAction::CloseoutLane,
            30,
        ),
    ]);

    let actions = engine.evaluate(&context);
    assert_eq!(
        actions,
        vec![
            PolicyAction::Reconcile {
                reason: ReconcileReason::AlreadyMerged,
            },
            PolicyAction::CloseoutLane,
        ]
    );
}

/// stale_branch::apply_policy: AutoRebase yields Rebase action.
#[test]
fn stale_branch_apply_policy_produces_rebase_action() {
    let stale = BranchFreshness::Stale {
        commits_behind: 5,
        missing_fixes: vec!["fix-123".to_string()],
    };

    let action = apply_policy(&stale, StaleBranchPolicy::AutoRebase);
    assert_eq!(action, StaleBranchAction::Rebase);
}

/// stale_branch::apply_policy: AutoMergeForward yields MergeForward action.
#[test]
fn stale_branch_apply_policy_produces_merge_forward_action() {
    let stale = BranchFreshness::Stale {
        commits_behind: 3,
        missing_fixes: vec![],
    };

    let action = apply_policy(&stale, StaleBranchPolicy::AutoMergeForward);
    assert_eq!(action, StaleBranchAction::MergeForward);
}

/// stale_branch::apply_policy: WarnOnly produces a Warn action with detail.
#[test]
fn stale_branch_apply_policy_warn_only() {
    let stale = BranchFreshness::Stale {
        commits_behind: 2,
        missing_fixes: vec!["fix-456".to_string()],
    };

    let action = apply_policy(&stale, StaleBranchPolicy::WarnOnly);
    match action {
        StaleBranchAction::Warn { message } => {
            assert!(message.contains("2 commit(s) behind main"));
            assert!(message.contains("fix-456"));
        }
        other => panic!("expected Warn action, got {:?}", other),
    }
}

/// stale_branch::apply_policy: Fresh branch produces Noop regardless of policy.
#[test]
fn stale_branch_fresh_produces_noop() {
    let fresh = BranchFreshness::Fresh;
    let action = apply_policy(&fresh, StaleBranchPolicy::AutoRebase);
    assert_eq!(action, StaleBranchAction::Noop);
}

/// End-to-end: stale + approved lane gets MergeForward + Notify in priority order.
#[test]
fn end_to_end_stale_lane_gets_merge_forward_action() {
    let context = LaneContext::new(
        "lane-9411",
        3,
        Duration::from_secs(5 * 60 * 60),
        LaneBlocker::None,
        ReviewStatus::Approved,
        DiffScope::Scoped,
        false,
    );

    let engine = PolicyEngine::new(vec![
        PolicyRule::new(
            "auto-merge-forward-if-stale-and-approved",
            PolicyCondition::And(vec![
                PolicyCondition::StaleBranch,
                PolicyCondition::ReviewPassed,
            ]),
            PolicyAction::MergeForward,
            5,
        ),
        PolicyRule::new(
            "stale-warning",
            PolicyCondition::StaleBranch,
            PolicyAction::Notify {
                channel: "#build-status".to_string(),
            },
            10,
        ),
    ]);

    let actions = engine.evaluate(&context);
    assert_eq!(
        actions,
        vec![
            PolicyAction::MergeForward,
            PolicyAction::Notify {
                channel: "#build-status".to_string(),
            },
        ]
    );
}

/// Fresh + approved lane bypasses stale handling and merges directly.
#[test]
fn fresh_approved_lane_gets_merge_action() {
    let context = LaneContext::new(
        "fresh-approved-lane",
        3,
        Duration::from_secs(30 * 60),
        LaneBlocker::None,
        ReviewStatus::Approved,
        DiffScope::Scoped,
        false,
    );

    let engine = PolicyEngine::new(vec![PolicyRule::new(
        "merge-if-green-approved-not-stale",
        PolicyCondition::And(vec![
            PolicyCondition::GreenAt { level: 3 },
            PolicyCondition::ReviewPassed,
        ]),
        PolicyAction::MergeToDev,
        5,
    )]);

    let actions = engine.evaluate(&context);
    assert_eq!(actions, vec![PolicyAction::MergeToDev]);
}

/// recovery_recipes + policy_engine integration:
/// A provider-failure WorkerFailureKind maps to a FailureScenario that
/// recovers in one step, after which the lane satisfies a green+approved
/// policy rule. Upstream test additionally drove a `WorkerRegistry` to
/// produce the `WorkerFailureKind`; that plumbing is out of crate scope
/// here, so the test enters the recovery path directly with the kind.
#[test]
fn worker_provider_failure_flows_through_recovery_to_policy() {
    let kind = WorkerFailureKind::Provider;

    let scenario = FailureScenario::from_worker_failure_kind(kind);
    assert_eq!(scenario, FailureScenario::ProviderFailure);

    let mut ctx = RecoveryContext::new();
    let result = attempt_recovery(&scenario, &mut ctx);

    assert!(
        matches!(result, RecoveryResult::Recovered { steps_taken: 1 }),
        "provider failure should recover via single RestartWorker step, got: {:?}",
        result
    );
    assert!(
        ctx.events().iter().any(|event| matches!(
            event,
            RecoveryEvent::RecoveryAttempted {
                result: RecoveryResult::Recovered { steps_taken: 1 },
                ..
            }
        )),
        "recovery should emit structured attempt event"
    );

    let recovery_success = matches!(result, RecoveryResult::Recovered { .. });
    let post_recovery_context = LaneContext::new(
        "recovered-lane",
        3,
        Duration::from_secs(30 * 60),
        LaneBlocker::None,
        ReviewStatus::Approved,
        DiffScope::Scoped,
        false,
    );

    let policy_engine = PolicyEngine::new(vec![PolicyRule::new(
        "merge-after-successful-recovery",
        PolicyCondition::And(vec![
            PolicyCondition::GreenAt { level: 3 },
            PolicyCondition::ReviewPassed,
        ]),
        PolicyAction::MergeToDev,
        10,
    )]);

    assert!(
        recovery_success,
        "recovery must succeed for lane to proceed"
    );
    let actions = policy_engine.evaluate(&post_recovery_context);
    assert_eq!(
        actions,
        vec![PolicyAction::MergeToDev],
        "post-recovery green+approved lane should be merge-ready"
    );
}
