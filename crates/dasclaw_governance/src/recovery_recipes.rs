//! Recovery recipes for common failure scenarios.
//!
//! Ported from `claw-code/rust/crates/runtime/src/recovery_recipes.rs` (W4 #66).
//!
//! Encodes known automatic recoveries for the seven failure scenarios in the
//! upstream ROADMAP item 8, and enforces **one** automatic recovery attempt
//! before escalation (governance contract: never silently retry forever). Each
//! attempt is emitted as a structured [`RecoveryEvent`] for audit.
//!
//! # Inputs
//!
//! [`WorkerFailureKind`] is inlined here as the recovery module's stable
//! input contract. The upstream `runtime::worker_boot::WorkerFailureKind`
//! type doesn't exist in x-claw yet; defining the 5-variant enum locally
//! keeps `dasclaw_governance` free of any cross-crate dependency on a
//! runtime/worker crate. Same pattern as `dasclaw_bash_validation::permissions`.
//!
//! # Outputs
//!
//! - [`RecoveryResult`] — three terminal states: Recovered / PartialRecovery / EscalationRequired
//! - [`RecoveryEvent`] — structured audit log entries (serde-serializable)

#![allow(clippy::cast_possible_truncation)]

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Worker boot failure category recovery_recipes consumes.
///
/// Inlined input contract — minimal subset needed by
/// [`FailureScenario::from_worker_failure_kind`]. Upstream source's
/// `runtime::worker_boot::WorkerFailureKind` carries the same 5 variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerFailureKind {
    /// Trust prompt was not resolved (auto-trust + allowlist both missed).
    TrustGate,
    /// Prompt did not reach the agent.
    PromptDelivery,
    /// Protocol-level failure (handshake / message format).
    Protocol,
    /// LLM provider unreachable / errored.
    Provider,
    /// Worker started but emitted no progress evidence.
    StartupNoEvidence,
}

/// The seven failure scenarios that have known recovery recipes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureScenario {
    TrustPromptUnresolved,
    PromptMisdelivery,
    StaleBranch,
    CompileRedCrossCrate,
    McpHandshakeFailure,
    PartialPluginStartup,
    ProviderFailure,
}

impl FailureScenario {
    /// Returns all known failure scenarios in declaration order.
    #[must_use]
    pub fn all() -> &'static [FailureScenario] {
        &[
            Self::TrustPromptUnresolved,
            Self::PromptMisdelivery,
            Self::StaleBranch,
            Self::CompileRedCrossCrate,
            Self::McpHandshakeFailure,
            Self::PartialPluginStartup,
            Self::ProviderFailure,
        ]
    }

    /// Map a [`WorkerFailureKind`] to the corresponding [`FailureScenario`].
    /// This is the bridge that lets recovery policy consume worker boot events.
    #[must_use]
    pub fn from_worker_failure_kind(kind: WorkerFailureKind) -> Self {
        match kind {
            WorkerFailureKind::TrustGate => Self::TrustPromptUnresolved,
            WorkerFailureKind::PromptDelivery => Self::PromptMisdelivery,
            WorkerFailureKind::Protocol => Self::McpHandshakeFailure,
            WorkerFailureKind::Provider | WorkerFailureKind::StartupNoEvidence => {
                Self::ProviderFailure
            }
        }
    }
}

impl std::fmt::Display for FailureScenario {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TrustPromptUnresolved => write!(f, "trust_prompt_unresolved"),
            Self::PromptMisdelivery => write!(f, "prompt_misdelivery"),
            Self::StaleBranch => write!(f, "stale_branch"),
            Self::CompileRedCrossCrate => write!(f, "compile_red_cross_crate"),
            Self::McpHandshakeFailure => write!(f, "mcp_handshake_failure"),
            Self::PartialPluginStartup => write!(f, "partial_plugin_startup"),
            Self::ProviderFailure => write!(f, "provider_failure"),
        }
    }
}

/// Individual step that can be executed as part of a recovery recipe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryStep {
    AcceptTrustPrompt,
    RedirectPromptToAgent,
    RebaseBranch,
    CleanBuild,
    RetryMcpHandshake { timeout: u64 },
    RestartPlugin { name: String },
    RestartWorker,
    EscalateToHuman { reason: String },
}

/// Policy governing what happens when automatic recovery is exhausted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EscalationPolicy {
    AlertHuman,
    LogAndContinue,
    Abort,
}

/// A recovery recipe encodes the sequence of steps to attempt for a
/// given failure scenario, along with the maximum number of automatic
/// attempts and the escalation policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryRecipe {
    pub scenario: FailureScenario,
    pub steps: Vec<RecoveryStep>,
    pub max_attempts: u32,
    pub escalation_policy: EscalationPolicy,
}

/// Outcome of a recovery attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryResult {
    Recovered {
        steps_taken: u32,
    },
    PartialRecovery {
        recovered: Vec<RecoveryStep>,
        remaining: Vec<RecoveryStep>,
    },
    EscalationRequired {
        reason: String,
    },
}

/// Structured event emitted during recovery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryEvent {
    RecoveryAttempted {
        scenario: FailureScenario,
        recipe: RecoveryRecipe,
        result: RecoveryResult,
    },
    RecoverySucceeded,
    RecoveryFailed,
    Escalated,
}

/// Minimal context for tracking recovery state and emitting events.
///
/// Holds per-scenario attempt counts, a structured event log, and an
/// optional simulation knob for controlling step outcomes during tests.
#[derive(Debug, Clone, Default)]
pub struct RecoveryContext {
    attempts: HashMap<FailureScenario, u32>,
    events: Vec<RecoveryEvent>,
    /// Optional step index at which simulated execution fails.
    /// `None` means all steps succeed.
    fail_at_step: Option<usize>,
}

impl RecoveryContext {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Configure a step index at which simulated execution will fail.
    #[must_use]
    pub fn with_fail_at_step(mut self, index: usize) -> Self {
        self.fail_at_step = Some(index);
        self
    }

    /// Returns the structured event log populated during recovery.
    #[must_use]
    pub fn events(&self) -> &[RecoveryEvent] {
        &self.events
    }

    /// Returns the number of recovery attempts made for a scenario.
    #[must_use]
    pub fn attempt_count(&self, scenario: &FailureScenario) -> u32 {
        self.attempts.get(scenario).copied().unwrap_or(0)
    }
}

/// Returns the known recovery recipe for the given failure scenario.
#[must_use]
pub fn recipe_for(scenario: &FailureScenario) -> RecoveryRecipe {
    match scenario {
        FailureScenario::TrustPromptUnresolved => RecoveryRecipe {
            scenario: *scenario,
            steps: vec![RecoveryStep::AcceptTrustPrompt],
            max_attempts: 1,
            escalation_policy: EscalationPolicy::AlertHuman,
        },
        FailureScenario::PromptMisdelivery => RecoveryRecipe {
            scenario: *scenario,
            steps: vec![RecoveryStep::RedirectPromptToAgent],
            max_attempts: 1,
            escalation_policy: EscalationPolicy::AlertHuman,
        },
        FailureScenario::StaleBranch => RecoveryRecipe {
            scenario: *scenario,
            steps: vec![RecoveryStep::RebaseBranch, RecoveryStep::CleanBuild],
            max_attempts: 1,
            escalation_policy: EscalationPolicy::AlertHuman,
        },
        FailureScenario::CompileRedCrossCrate => RecoveryRecipe {
            scenario: *scenario,
            steps: vec![RecoveryStep::CleanBuild],
            max_attempts: 1,
            escalation_policy: EscalationPolicy::AlertHuman,
        },
        FailureScenario::McpHandshakeFailure => RecoveryRecipe {
            scenario: *scenario,
            steps: vec![RecoveryStep::RetryMcpHandshake { timeout: 5000 }],
            max_attempts: 1,
            escalation_policy: EscalationPolicy::Abort,
        },
        FailureScenario::PartialPluginStartup => RecoveryRecipe {
            scenario: *scenario,
            steps: vec![
                RecoveryStep::RestartPlugin {
                    name: "stalled".to_string(),
                },
                RecoveryStep::RetryMcpHandshake { timeout: 3000 },
            ],
            max_attempts: 1,
            escalation_policy: EscalationPolicy::LogAndContinue,
        },
        FailureScenario::ProviderFailure => RecoveryRecipe {
            scenario: *scenario,
            steps: vec![RecoveryStep::RestartWorker],
            max_attempts: 1,
            escalation_policy: EscalationPolicy::AlertHuman,
        },
    }
}

/// Attempts automatic recovery for the given failure scenario.
///
/// Looks up the recipe, enforces the one-attempt-before-escalation policy,
/// simulates step execution (controlled by the context), and emits structured
/// [`RecoveryEvent`]s for every attempt.
pub fn attempt_recovery(scenario: &FailureScenario, ctx: &mut RecoveryContext) -> RecoveryResult {
    let recipe = recipe_for(scenario);
    let attempt_count = ctx.attempts.entry(*scenario).or_insert(0);

    // Enforce one automatic recovery attempt before escalation.
    if *attempt_count >= recipe.max_attempts {
        let result = RecoveryResult::EscalationRequired {
            reason: format!(
                "max recovery attempts ({}) exceeded for {}",
                recipe.max_attempts, scenario
            ),
        };
        ctx.events.push(RecoveryEvent::RecoveryAttempted {
            scenario: *scenario,
            recipe,
            result: result.clone(),
        });
        ctx.events.push(RecoveryEvent::Escalated);
        return result;
    }

    *attempt_count += 1;

    // Execute steps, honoring the optional fail_at_step simulation.
    let fail_index = ctx.fail_at_step;
    let mut executed = Vec::new();
    let mut failed = false;

    for (i, step) in recipe.steps.iter().enumerate() {
        if fail_index == Some(i) {
            failed = true;
            break;
        }
        executed.push(step.clone());
    }

    let result = if failed {
        let remaining: Vec<RecoveryStep> = recipe.steps[executed.len()..].to_vec();
        if executed.is_empty() {
            RecoveryResult::EscalationRequired {
                reason: format!("recovery failed at first step for {}", scenario),
            }
        } else {
            RecoveryResult::PartialRecovery {
                recovered: executed,
                remaining,
            }
        }
    } else {
        RecoveryResult::Recovered {
            steps_taken: recipe.steps.len() as u32,
        }
    };

    // Emit the attempt as structured event data.
    ctx.events.push(RecoveryEvent::RecoveryAttempted {
        scenario: *scenario,
        recipe,
        result: result.clone(),
    });

    match &result {
        RecoveryResult::Recovered { .. } => {
            ctx.events.push(RecoveryEvent::RecoverySucceeded);
        }
        RecoveryResult::PartialRecovery { .. } => {
            ctx.events.push(RecoveryEvent::RecoveryFailed);
        }
        RecoveryResult::EscalationRequired { .. } => {
            ctx.events.push(RecoveryEvent::Escalated);
        }
    }

    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_recovery_recipes_66_each_scenario_has_a_matching_recipe() {
        let scenarios = FailureScenario::all();

        for scenario in scenarios {
            let recipe = recipe_for(scenario);
            assert_eq!(
                recipe.scenario, *scenario,
                "recipe scenario should match requested scenario"
            );
            assert!(
                !recipe.steps.is_empty(),
                "recipe for {} should have at least one step",
                scenario
            );
            assert!(
                recipe.max_attempts >= 1,
                "recipe for {} should allow at least one attempt",
                scenario
            );
        }
    }

    #[test]
    fn req_recovery_recipes_66_successful_recovery_emits_two_events() {
        let mut ctx = RecoveryContext::new();
        let scenario = FailureScenario::TrustPromptUnresolved;

        let result = attempt_recovery(&scenario, &mut ctx);

        assert_eq!(result, RecoveryResult::Recovered { steps_taken: 1 });
        assert_eq!(ctx.events().len(), 2);
        assert!(matches!(
            &ctx.events()[0],
            RecoveryEvent::RecoveryAttempted {
                scenario: s,
                result: r,
                ..
            } if *s == FailureScenario::TrustPromptUnresolved
              && matches!(r, RecoveryResult::Recovered { steps_taken: 1 })
        ));
        assert_eq!(ctx.events()[1], RecoveryEvent::RecoverySucceeded);
    }

    #[test]
    fn req_recovery_recipes_66_escalation_after_max_attempts_exceeded() {
        let mut ctx = RecoveryContext::new();
        let scenario = FailureScenario::PromptMisdelivery;

        let first = attempt_recovery(&scenario, &mut ctx);
        assert!(matches!(first, RecoveryResult::Recovered { .. }));

        let second = attempt_recovery(&scenario, &mut ctx);

        assert!(
            matches!(
                &second,
                RecoveryResult::EscalationRequired { reason }
                    if reason.contains("max recovery attempts")
            ),
            "second attempt should require escalation, got: {second:?}"
        );
        // Counter is only incremented for actual attempts, not escalation-on-empty.
        assert_eq!(ctx.attempt_count(&scenario), 1);
        assert!(ctx
            .events()
            .iter()
            .any(|e| matches!(e, RecoveryEvent::Escalated)));
    }

    #[test]
    fn req_recovery_recipes_66_partial_recovery_when_step_fails_midway() {
        // PartialPluginStartup has two steps; fail at step index 1.
        let mut ctx = RecoveryContext::new().with_fail_at_step(1);
        let scenario = FailureScenario::PartialPluginStartup;

        let result = attempt_recovery(&scenario, &mut ctx);

        match &result {
            RecoveryResult::PartialRecovery {
                recovered,
                remaining,
            } => {
                assert_eq!(recovered.len(), 1, "one step should have succeeded");
                assert_eq!(remaining.len(), 1, "one step should remain");
                assert!(matches!(recovered[0], RecoveryStep::RestartPlugin { .. }));
                assert!(matches!(
                    remaining[0],
                    RecoveryStep::RetryMcpHandshake { .. }
                ));
            }
            other => panic!("expected PartialRecovery, got {other:?}"),
        }
        assert!(ctx
            .events()
            .iter()
            .any(|e| matches!(e, RecoveryEvent::RecoveryFailed)));
    }

    #[test]
    fn req_recovery_recipes_66_first_step_failure_escalates_immediately() {
        let mut ctx = RecoveryContext::new().with_fail_at_step(0);
        let scenario = FailureScenario::CompileRedCrossCrate;

        let result = attempt_recovery(&scenario, &mut ctx);

        assert!(
            matches!(
                &result,
                RecoveryResult::EscalationRequired { reason }
                    if reason.contains("failed at first step")
            ),
            "zero-step failure should escalate, got: {result:?}"
        );
        assert!(ctx
            .events()
            .iter()
            .any(|e| matches!(e, RecoveryEvent::Escalated)));
    }

    #[test]
    fn req_recovery_recipes_66_emitted_events_carry_full_recipe_context() {
        let mut ctx = RecoveryContext::new();
        let scenario = FailureScenario::McpHandshakeFailure;

        let _ = attempt_recovery(&scenario, &mut ctx);

        let attempted = ctx
            .events()
            .iter()
            .find(|e| matches!(e, RecoveryEvent::RecoveryAttempted { .. }))
            .expect("should have emitted RecoveryAttempted event");

        match attempted {
            RecoveryEvent::RecoveryAttempted {
                scenario: s,
                recipe,
                result,
            } => {
                assert_eq!(*s, scenario);
                assert_eq!(recipe.scenario, scenario);
                assert!(!recipe.steps.is_empty());
                assert!(matches!(result, RecoveryResult::Recovered { .. }));
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn req_recovery_recipes_66_recovery_context_tracks_attempts_per_scenario() {
        let mut ctx = RecoveryContext::new();

        assert_eq!(ctx.attempt_count(&FailureScenario::StaleBranch), 0);
        attempt_recovery(&FailureScenario::StaleBranch, &mut ctx);

        assert_eq!(ctx.attempt_count(&FailureScenario::StaleBranch), 1);
        assert_eq!(ctx.attempt_count(&FailureScenario::PromptMisdelivery), 0);
    }

    #[test]
    fn req_recovery_recipes_66_stale_branch_recipe_has_rebase_then_clean_build() {
        let recipe = recipe_for(&FailureScenario::StaleBranch);

        assert_eq!(recipe.steps.len(), 2);
        assert_eq!(recipe.steps[0], RecoveryStep::RebaseBranch);
        assert_eq!(recipe.steps[1], RecoveryStep::CleanBuild);
    }

    #[test]
    fn req_recovery_recipes_66_partial_plugin_startup_recipe_has_restart_then_handshake() {
        let recipe = recipe_for(&FailureScenario::PartialPluginStartup);

        assert_eq!(recipe.steps.len(), 2);
        assert!(matches!(
            recipe.steps[0],
            RecoveryStep::RestartPlugin { .. }
        ));
        assert!(matches!(
            recipe.steps[1],
            RecoveryStep::RetryMcpHandshake { timeout: 3000 }
        ));
        assert_eq!(recipe.escalation_policy, EscalationPolicy::LogAndContinue);
    }

    #[test]
    fn req_recovery_recipes_66_failure_scenario_display_all_variants() {
        let cases = [
            (
                FailureScenario::TrustPromptUnresolved,
                "trust_prompt_unresolved",
            ),
            (FailureScenario::PromptMisdelivery, "prompt_misdelivery"),
            (FailureScenario::StaleBranch, "stale_branch"),
            (
                FailureScenario::CompileRedCrossCrate,
                "compile_red_cross_crate",
            ),
            (
                FailureScenario::McpHandshakeFailure,
                "mcp_handshake_failure",
            ),
            (
                FailureScenario::PartialPluginStartup,
                "partial_plugin_startup",
            ),
            (FailureScenario::ProviderFailure, "provider_failure"),
        ];

        for (scenario, expected) in &cases {
            assert_eq!(scenario.to_string(), *expected);
        }
    }

    #[test]
    fn req_recovery_recipes_66_multi_step_success_reports_correct_steps_taken() {
        let mut ctx = RecoveryContext::new();
        let scenario = FailureScenario::StaleBranch;

        let result = attempt_recovery(&scenario, &mut ctx);

        assert_eq!(result, RecoveryResult::Recovered { steps_taken: 2 });
    }

    #[test]
    fn req_recovery_recipes_66_mcp_handshake_recipe_uses_abort_escalation_policy() {
        let recipe = recipe_for(&FailureScenario::McpHandshakeFailure);

        assert_eq!(recipe.escalation_policy, EscalationPolicy::Abort);
        assert_eq!(recipe.max_attempts, 1);
    }

    #[test]
    fn req_recovery_recipes_66_worker_failure_kind_maps_to_failure_scenario() {
        assert_eq!(
            FailureScenario::from_worker_failure_kind(WorkerFailureKind::TrustGate),
            FailureScenario::TrustPromptUnresolved,
        );
        assert_eq!(
            FailureScenario::from_worker_failure_kind(WorkerFailureKind::PromptDelivery),
            FailureScenario::PromptMisdelivery,
        );
        assert_eq!(
            FailureScenario::from_worker_failure_kind(WorkerFailureKind::Protocol),
            FailureScenario::McpHandshakeFailure,
        );
        assert_eq!(
            FailureScenario::from_worker_failure_kind(WorkerFailureKind::Provider),
            FailureScenario::ProviderFailure,
        );
        assert_eq!(
            FailureScenario::from_worker_failure_kind(WorkerFailureKind::StartupNoEvidence),
            FailureScenario::ProviderFailure,
        );
    }

    #[test]
    fn req_recovery_recipes_66_provider_failure_recipe_uses_restart_worker() {
        let recipe = recipe_for(&FailureScenario::ProviderFailure);

        assert_eq!(recipe.scenario, FailureScenario::ProviderFailure);
        assert!(recipe.steps.contains(&RecoveryStep::RestartWorker));
        assert_eq!(recipe.escalation_policy, EscalationPolicy::AlertHuman);
        assert_eq!(recipe.max_attempts, 1);
    }

    #[test]
    fn req_recovery_recipes_66_provider_failure_attempt_succeeds_then_escalates() {
        let mut ctx = RecoveryContext::new();
        let scenario = FailureScenario::ProviderFailure;

        let first = attempt_recovery(&scenario, &mut ctx);
        assert!(matches!(first, RecoveryResult::Recovered { .. }));

        let second = attempt_recovery(&scenario, &mut ctx);
        assert!(matches!(second, RecoveryResult::EscalationRequired { .. }));
        assert!(ctx
            .events()
            .iter()
            .any(|e| matches!(e, RecoveryEvent::Escalated)));
    }

    #[test]
    fn req_recovery_recipes_66_all_returns_seven_distinct_scenarios() {
        let all = FailureScenario::all();
        assert_eq!(all.len(), 7);

        // Distinct
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                assert_ne!(a, b, "FailureScenario::all() must contain distinct values");
            }
        }
    }
}
