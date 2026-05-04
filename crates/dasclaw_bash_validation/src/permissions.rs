//! Permission mode for bash command validation.
//!
//! Self-contained port of `claw-code/rust/crates/runtime/src/permissions.rs`'s
//! `PermissionMode` enum. Kept inside this crate so `dasclaw_bash_validation`
//! has no cross-crate dependency on `dasclaw_governance` or runtime types.

/// Permission level assigned to a tool invocation or runtime session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PermissionMode {
    /// No filesystem or state mutations allowed.
    ReadOnly,
    /// Writes inside the workspace allowed; system-level writes warn.
    WorkspaceWrite,
    /// Full host access — bypass most validations.
    DangerFullAccess,
    /// Each invocation prompts the user.
    Prompt,
    /// Pre-approved invocation, treat as full access.
    Allow,
}

impl PermissionMode {
    /// Stable string identifier used by config / audit logs.
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
    fn req_bash_validation_72_permission_mode_as_str_stable() {
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
    fn req_bash_validation_72_permission_mode_ordering() {
        assert!(PermissionMode::ReadOnly < PermissionMode::WorkspaceWrite);
        assert!(PermissionMode::WorkspaceWrite < PermissionMode::DangerFullAccess);
    }
}
