//! Tool registration report — startup-time visibility into which tools were
//! accepted, rejected, or shadowed by the registry.
//!
//! Issue #88 (P1/W5): "Tool collision and visibility startup report".
//!
//! # Scope (this PR — builtin-only first cut)
//!
//! Records every call to [`crate::tools::registry::ToolRegistry::register`]
//! and [`crate::tools::registry::ToolRegistry::register_sync`] with:
//! - canonical tool name
//! - source bucket ([`ToolSource`])
//! - outcome ([`RegistrationOutcome`]) including rejection reason
//!
//! The combined report ([`ToolRegistrationReport`]) is rendered at startup as
//! a structured `tracing::info!` event so operators can see registry shape and
//! shadow-rejections at a glance, and can be consumed by tests / CI checks
//! without leaking secrets (no tool params or secret values are recorded).
//!
//! # Out of scope (follow-up issues)
//!
//! - Per-source breakdowns for MCP / WASM / extension tools beyond the
//!   `Builtin` / `Dynamic` split (those flow through `register` today and are
//!   labelled `Dynamic`; finer attribution will land alongside per-source
//!   call-site changes).
//! - Display name / approval mode / sandbox mode / risk classification
//!   columns — those require touching every `Tool` impl and belong in a
//!   separate PR (issue #88 acceptance allows starting with builtin-only
//!   reporting).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Where a tool registration originated from.
///
/// Only the two values produced by the current registry entry points are
/// modelled. New variants are non-breaking additions thanks to
/// `#[non_exhaustive]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ToolSource {
    /// Registered through the synchronous startup path
    /// ([`crate::tools::registry::ToolRegistry::register_sync`]).
    /// Includes all built-in groups (echo/time/json/http, dev tools, memory,
    /// job, message, extension/skill/routine/image/vision/secrets, tool_info).
    Builtin,
    /// Registered through the dynamic async path
    /// ([`crate::tools::registry::ToolRegistry::register`]). Today this
    /// covers WASM tools, the software-builder tool, and any other
    /// programmatically-added tool.
    Dynamic,
}

impl ToolSource {
    /// Stable string label suitable for log fields and JSON keys.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Builtin => "builtin",
            Self::Dynamic => "dynamic",
        }
    }
}

/// Why a registration attempt was rejected.
///
/// Only one variant exists today (shadowing a protected built-in). Marked
/// `#[non_exhaustive]` so additional reasons (policy / capability / signature
/// failure) can land without breaking external matches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum RejectionReason {
    /// A dynamic tool tried to register under a name owned by a protected
    /// built-in (see `PROTECTED_TOOL_NAMES` in registry.rs). The dynamic
    /// registration is dropped to keep security-critical tools (`shell`,
    /// `memory_write`, …) un-shadowable.
    ProtectedBuiltinShadow,
}

impl RejectionReason {
    /// Stable string suitable for log fields and test fixtures.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ProtectedBuiltinShadow => "protected_builtin_shadow",
        }
    }
}

/// Outcome of one registration attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RegistrationOutcome {
    /// The tool was inserted into the registry.
    Accepted,
    /// The tool was dropped before insertion.
    Rejected { reason: RejectionReason },
}

impl RegistrationOutcome {
    /// `true` when the tool ended up in the registry.
    pub fn is_accepted(&self) -> bool {
        matches!(self, Self::Accepted)
    }
}

/// One entry in the registration ledger.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolRegistrationEntry {
    /// Canonical tool name (the value returned by `Tool::name()` at
    /// registration time).
    pub name: String,
    /// Source bucket the registration came from.
    pub source: ToolSource,
    /// Whether the registration was accepted or rejected (with reason).
    pub outcome: RegistrationOutcome,
}

/// Aggregated report consumed by tests, CI checks, and the startup
/// `tracing::info!` event.
///
/// The struct is intentionally small and `Serialize`-able so callers can
/// snapshot it as JSON for golden-file tests without depending on internal
/// registry types.
///
/// Sort order is canonical: `accepted` lists are sorted by name and then by
/// source label, and `rejected` is sorted by name. Tests can assert on the
/// report without observing insertion order, satisfying the issue #88
/// "deterministic, brittle-order-free" acceptance bullet.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolRegistrationReport {
    /// Names accepted into the registry, grouped by [`ToolSource`].
    /// Maps source label (`"builtin"`, `"dynamic"`) to a sorted, deduped
    /// list of tool names.
    pub accepted: BTreeMap<String, Vec<String>>,
    /// Tools whose registration attempt was rejected, sorted by name.
    pub rejected: Vec<RejectedToolEntry>,
    /// Tools that are registered but disabled by feature-flag policy.
    /// Populated only when [`ToolRegistry::registration_report`] is called
    /// with a [`crate::tools::feature_flags::ToolFeatureFlags`] argument;
    /// otherwise empty.
    pub policy_disabled: Vec<String>,
}

/// Compact rejection record exposed in [`ToolRegistrationReport`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RejectedToolEntry {
    pub name: String,
    pub source: ToolSource,
    pub reason: RejectionReason,
}

impl ToolRegistrationReport {
    /// Build a report from a flat ledger plus the optional feature-flag view.
    ///
    /// `entries` is consumed; sort order of the input is irrelevant.
    pub fn from_entries(entries: &[ToolRegistrationEntry], policy_disabled: Vec<String>) -> Self {
        let mut accepted: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut rejected: Vec<RejectedToolEntry> = Vec::new();

        for entry in entries {
            match &entry.outcome {
                RegistrationOutcome::Accepted => {
                    accepted
                        .entry(entry.source.as_str().to_string())
                        .or_default()
                        .push(entry.name.clone());
                }
                RegistrationOutcome::Rejected { reason } => {
                    rejected.push(RejectedToolEntry {
                        name: entry.name.clone(),
                        source: entry.source,
                        reason: reason.clone(),
                    });
                }
            }
        }

        for names in accepted.values_mut() {
            names.sort_unstable();
            names.dedup();
        }

        rejected.sort_by(|a, b| a.name.cmp(&b.name));
        rejected.dedup();

        let mut policy_disabled = policy_disabled;
        policy_disabled.sort_unstable();
        policy_disabled.dedup();

        Self {
            accepted,
            rejected,
            policy_disabled,
        }
    }

    /// Total accepted tool count across all sources.
    pub fn accepted_count(&self) -> usize {
        self.accepted.values().map(|v| v.len()).sum()
    }

    /// Render a single human-readable summary line, e.g.
    /// `"tools: 42 accepted (builtin=40 dynamic=2), 1 rejected, 3 policy-disabled"`.
    pub fn summary_line(&self) -> String {
        let mut parts: Vec<String> = self
            .accepted
            .iter()
            .map(|(src, names)| format!("{}={}", src, names.len()))
            .collect();
        parts.sort();
        let breakdown = if parts.is_empty() {
            String::new()
        } else {
            format!(" ({})", parts.join(" "))
        };
        format!(
            "tools: {} accepted{}, {} rejected, {} policy-disabled",
            self.accepted_count(),
            breakdown,
            self.rejected.len(),
            self.policy_disabled.len(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(
        name: &str,
        source: ToolSource,
        outcome: RegistrationOutcome,
    ) -> ToolRegistrationEntry {
        ToolRegistrationEntry {
            name: name.to_string(),
            source,
            outcome,
        }
    }

    fn accepted(name: &str, source: ToolSource) -> ToolRegistrationEntry {
        entry(name, source, RegistrationOutcome::Accepted)
    }

    fn rejected(name: &str, source: ToolSource, reason: RejectionReason) -> ToolRegistrationEntry {
        entry(name, source, RegistrationOutcome::Rejected { reason })
    }

    #[test]
    fn req_88_accepted_grouped_by_source_and_sorted() {
        let entries = vec![
            accepted("zeta", ToolSource::Builtin),
            accepted("alpha", ToolSource::Builtin),
            accepted("dyn_b", ToolSource::Dynamic),
            accepted("dyn_a", ToolSource::Dynamic),
        ];

        let report = ToolRegistrationReport::from_entries(&entries, vec![]);

        assert_eq!(
            report.accepted.get("builtin").map(|v| v.as_slice()),
            Some(["alpha".to_string(), "zeta".to_string()].as_slice())
        );
        assert_eq!(
            report.accepted.get("dynamic").map(|v| v.as_slice()),
            Some(["dyn_a".to_string(), "dyn_b".to_string()].as_slice())
        );
        assert_eq!(report.accepted_count(), 4);
    }

    #[test]
    fn req_88_rejection_records_deterministic_reason() {
        let entries = vec![
            rejected(
                "shell",
                ToolSource::Dynamic,
                RejectionReason::ProtectedBuiltinShadow,
            ),
            accepted("shell", ToolSource::Builtin),
        ];

        let report = ToolRegistrationReport::from_entries(&entries, vec![]);

        assert_eq!(report.rejected.len(), 1);
        let r = &report.rejected[0];
        assert_eq!(r.name, "shell");
        assert_eq!(r.source, ToolSource::Dynamic);
        assert_eq!(r.reason, RejectionReason::ProtectedBuiltinShadow);
        assert_eq!(r.reason.as_str(), "protected_builtin_shadow");

        // Accepted entry for the same name still tracked separately —
        // protected built-in is in the registry, dynamic was dropped.
        assert!(
            report
                .accepted
                .get("builtin")
                .is_some_and(|v| v.contains(&"shell".to_string()))
        );
    }

    #[test]
    fn req_88_policy_disabled_sorted_and_deduped() {
        let entries = vec![
            accepted("shell", ToolSource::Builtin),
            accepted("http", ToolSource::Builtin),
        ];
        let report = ToolRegistrationReport::from_entries(
            &entries,
            vec!["shell".into(), "http".into(), "shell".into()],
        );

        assert_eq!(
            report.policy_disabled,
            vec!["http".to_string(), "shell".to_string()]
        );
    }

    #[test]
    fn req_88_summary_line_shape_does_not_leak_internals() {
        let entries = vec![
            accepted("a", ToolSource::Builtin),
            accepted("b", ToolSource::Builtin),
            rejected(
                "shell",
                ToolSource::Dynamic,
                RejectionReason::ProtectedBuiltinShadow,
            ),
        ];
        let report = ToolRegistrationReport::from_entries(&entries, vec!["b".into()]);

        let line = report.summary_line();
        // Counts only — no tool names, parameters, or secrets.
        assert_eq!(
            line,
            "tools: 2 accepted (builtin=2), 1 rejected, 1 policy-disabled"
        );
    }

    #[test]
    fn req_88_test_fixture_is_order_independent() {
        // Two ledger orderings that should produce identical reports.
        let a = vec![
            accepted("alpha", ToolSource::Builtin),
            accepted("beta", ToolSource::Builtin),
            rejected(
                "alpha",
                ToolSource::Dynamic,
                RejectionReason::ProtectedBuiltinShadow,
            ),
        ];
        let b = vec![
            rejected(
                "alpha",
                ToolSource::Dynamic,
                RejectionReason::ProtectedBuiltinShadow,
            ),
            accepted("beta", ToolSource::Builtin),
            accepted("alpha", ToolSource::Builtin),
        ];

        let ra = ToolRegistrationReport::from_entries(&a, vec![]);
        let rb = ToolRegistrationReport::from_entries(&b, vec![]);

        assert_eq!(ra.accepted, rb.accepted);
        assert_eq!(ra.rejected, rb.rejected);
    }
}
