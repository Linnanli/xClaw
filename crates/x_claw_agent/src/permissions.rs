//! Permission mode used by the bash validation pipeline.
//!
//! This module is a **deliberately sliced port** of upstream claw-code's
//! `runtime/src/permissions.rs`. Only the `PermissionMode` enum itself is
//! carried over — the full `PermissionPolicy` / `PermissionContext` /
//! `PermissionRequest` / `PermissionOverride` / `PermissionOutcome`
//! machinery and the `RuntimePermissionRuleConfig` dependency are
//! intentionally left behind.
//!
//! Rationale:
//!
//! - `bash_validation` is the only current consumer and it uses nothing
//!   beyond the 5-variant enum + `Copy`/`Eq`/`as_str()`.
//! - Pulling the full policy engine would drag `RuntimePermissionRuleConfig`
//!   and the upstream `config` module into `x_claw_agent`, which Phase 3
//!   explicitly scope-narrowed away (see `UPSTREAM_BASELINE.md` Step D-3).
//! - The broader permission-policy story belongs to ironclaw's existing
//!   `safety` + `tools` + `extensions` surface (policy, DLP, denylist,
//!   approval flow), not to the agent kernel.
//!
//! When a concrete need arises to run upstream's full policy engine, this
//! module can grow — but the cherry-pick drill (Step K) exists precisely
//! to prove the "port-small-and-documented-deviation" workflow.

/// Permission level assigned to a tool invocation or runtime session.
///
/// Variant set mirrors upstream verbatim so semantic comparisons
/// (`==`/`!=`) in `bash_validation` behave identically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PermissionMode {
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
    Prompt,
    Allow,
}

impl PermissionMode {
    /// Stable string representation, matching upstream `as_str`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ReadOnly => "read-only",
            Self::WorkspaceWrite => "workspace-write",
            Self::DangerFullAccess => "danger-full-access",
            Self::Prompt => "prompt",
            Self::Allow => "allow",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_str_is_stable_for_all_variants() {
        assert_eq!(PermissionMode::ReadOnly.as_str(), "read-only");
        assert_eq!(PermissionMode::WorkspaceWrite.as_str(), "workspace-write");
        assert_eq!(
            PermissionMode::DangerFullAccess.as_str(),
            "danger-full-access"
        );
        assert_eq!(PermissionMode::Prompt.as_str(), "prompt");
        assert_eq!(PermissionMode::Allow.as_str(), "allow");
    }

    #[test]
    fn ordering_matches_ascending_privilege() {
        // Upstream derives PartialOrd/Ord on declaration order:
        // ReadOnly < WorkspaceWrite < DangerFullAccess < Prompt < Allow.
        assert!(PermissionMode::ReadOnly < PermissionMode::WorkspaceWrite);
        assert!(PermissionMode::WorkspaceWrite < PermissionMode::DangerFullAccess);
    }
}
