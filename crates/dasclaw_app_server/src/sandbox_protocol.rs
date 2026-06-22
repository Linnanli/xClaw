use std::path::Path;

use dasclaw_app_server_protocol::SandboxMode;
use dasclaw_protocol::models::PermissionProfile;
use dasclaw_protocol::protocol as codex_protocol;
use dasclaw_workspace_cap::policy::{
    NetworkAccess as WorkspaceNetworkAccess, SandboxPolicy as WorkspaceSandboxPolicy,
};
use serde_json::Value;

use crate::AppServerError;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeSandboxContext {
    pub sandbox: Option<SandboxMode>,
    pub policy: Option<WorkspaceSandboxPolicy>,
}

impl RuntimeSandboxContext {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.sandbox.is_none() && self.policy.is_none()
    }
}

pub fn resolve_thread_context(
    sandbox: Option<SandboxMode>,
    permission_profile: Option<Value>,
    cwd: &Path,
    capability: &'static str,
) -> Result<RuntimeSandboxContext, AppServerError> {
    if sandbox.is_some() && permission_profile.is_some() {
        return Err(AppServerError::invalid_request(
            capability,
            "choose either sandbox or permissionProfile, not both",
        ));
    }

    let policy = match (sandbox.as_ref(), permission_profile) {
        (Some(mode), None) => Some(policy_from_mode(*mode)),
        (None, Some(value)) => Some(policy_from_permission_profile(value, cwd, capability)?),
        (None, None) => None,
        (Some(_), Some(_)) => unreachable!("checked above"),
    };
    if let Some(policy) = policy.as_ref() {
        reject_thread_turn_full_access(policy, capability)?;
    }

    Ok(RuntimeSandboxContext { sandbox, policy })
}

pub fn resolve_turn_context(
    sandbox_policy: Option<Value>,
    permission_profile: Option<Value>,
    fallback: RuntimeSandboxContext,
    cwd: &Path,
    capability: &'static str,
) -> Result<RuntimeSandboxContext, AppServerError> {
    let Some(policy) =
        resolve_policy_override(sandbox_policy, permission_profile, cwd, capability)?
    else {
        return Ok(fallback);
    };

    reject_thread_turn_full_access(&policy, capability)?;
    Ok(RuntimeSandboxContext {
        sandbox: None,
        policy: Some(policy),
    })
}

pub fn resolve_policy_override(
    sandbox_policy: Option<Value>,
    permission_profile: Option<Value>,
    cwd: &Path,
    capability: &'static str,
) -> Result<Option<WorkspaceSandboxPolicy>, AppServerError> {
    match (sandbox_policy, permission_profile) {
        (Some(_), Some(_)) => Err(AppServerError::invalid_request(
            capability,
            "choose either sandboxPolicy or permissionProfile, not both",
        )),
        (Some(value), None) => {
            let policy = serde_json::from_value::<codex_protocol::SandboxPolicy>(value).map_err(
                |error| {
                    AppServerError::invalid_request(
                        capability,
                        format!("invalid sandboxPolicy: {error}"),
                    )
                },
            )?;
            Ok(Some(workspace_policy_from_codex(policy)))
        }
        (None, Some(value)) => Ok(Some(policy_from_permission_profile(
            value, cwd, capability,
        )?)),
        (None, None) => Ok(None),
    }
}

fn reject_thread_turn_full_access(
    policy: &WorkspaceSandboxPolicy,
    capability: &'static str,
) -> Result<(), AppServerError> {
    if matches!(policy, WorkspaceSandboxPolicy::DangerFullAccess) {
        return Err(AppServerError::invalid_request(
            capability,
            "danger-full-access sandbox mode is not supported for thread/turn runtime context",
        ));
    }

    Ok(())
}

fn policy_from_mode(mode: SandboxMode) -> WorkspaceSandboxPolicy {
    match mode {
        SandboxMode::ReadOnly => WorkspaceSandboxPolicy::new_read_only_policy(),
        SandboxMode::WorkspaceWrite => WorkspaceSandboxPolicy::new_workspace_write_policy(),
        SandboxMode::DangerFullAccess => WorkspaceSandboxPolicy::DangerFullAccess,
    }
}

fn policy_from_permission_profile(
    value: Value,
    cwd: &Path,
    capability: &'static str,
) -> Result<WorkspaceSandboxPolicy, AppServerError> {
    let profile = serde_json::from_value::<PermissionProfile>(value).map_err(|error| {
        AppServerError::invalid_request(capability, format!("invalid permissionProfile: {error}"))
    })?;
    let legacy = profile.to_legacy_sandbox_policy(cwd).map_err(|error| {
        AppServerError::invalid_request(
            capability,
            format!("invalid permissionProfile path policy: {error}"),
        )
    })?;
    Ok(workspace_policy_from_codex(legacy))
}

fn workspace_policy_from_codex(policy: codex_protocol::SandboxPolicy) -> WorkspaceSandboxPolicy {
    match policy {
        codex_protocol::SandboxPolicy::DangerFullAccess => WorkspaceSandboxPolicy::DangerFullAccess,
        codex_protocol::SandboxPolicy::ReadOnly { network_access } => {
            WorkspaceSandboxPolicy::ReadOnly { network_access }
        }
        codex_protocol::SandboxPolicy::ExternalSandbox { network_access } => {
            WorkspaceSandboxPolicy::ExternalSandbox {
                network_access: match network_access {
                    codex_protocol::NetworkAccess::Restricted => WorkspaceNetworkAccess::Restricted,
                    codex_protocol::NetworkAccess::Enabled => WorkspaceNetworkAccess::Enabled,
                },
            }
        }
        codex_protocol::SandboxPolicy::WorkspaceWrite {
            writable_roots,
            network_access,
            exclude_tmpdir_env_var,
            exclude_slash_tmp,
        } => WorkspaceSandboxPolicy::WorkspaceWrite {
            writable_roots: writable_roots.into_iter().map(Into::into).collect(),
            network_access,
            exclude_tmpdir_env_var,
            exclude_slash_tmp,
        },
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use dasclaw_app_server_protocol::SandboxMode;
    use dasclaw_workspace_cap::policy::SandboxPolicy;
    use serde_json::json;

    use super::{resolve_policy_override, resolve_thread_context};

    #[test]
    fn resolves_command_sandbox_policy_json_to_workspace_policy() {
        let policy = resolve_policy_override(
            Some(json!({
                "type": "workspace-write",
                "writable_roots": ["/tmp/extra"],
                "network_access": true,
                "exclude_tmpdir_env_var": true,
                "exclude_slash_tmp": true
            })),
            None,
            Path::new("/repo"),
            "command/exec",
        )
        .expect("resolve")
        .expect("policy");

        assert_eq!(
            policy,
            SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![std::path::PathBuf::from("/tmp/extra")],
                network_access: true,
                exclude_tmpdir_env_var: true,
                exclude_slash_tmp: true,
            }
        );
    }

    #[test]
    fn rejects_ambiguous_policy_and_permission_profile() {
        let error = resolve_policy_override(
            Some(json!({ "type": "read-only" })),
            Some(json!({ "type": "disabled" })),
            Path::new("/repo"),
            "command/exec",
        )
        .expect_err("ambiguous override");

        assert!(
            error
                .public_message()
                .contains("choose either sandboxPolicy or permissionProfile"),
            "{error:?}"
        );
    }

    #[test]
    fn resolves_permission_profile_json_to_workspace_policy() {
        let policy = resolve_policy_override(
            None,
            Some(json!({ "type": "disabled" })),
            Path::new("/repo"),
            "command/exec",
        )
        .expect("resolve")
        .expect("policy");

        assert_eq!(policy, SandboxPolicy::DangerFullAccess);
    }

    #[test]
    fn rejects_invalid_permission_profile_json() {
        let error = resolve_policy_override(
            None,
            Some(json!({ "type": "unknown" })),
            Path::new("/repo"),
            "command/exec",
        )
        .expect_err("invalid profile");

        assert!(
            error.public_message().contains("invalid permissionProfile"),
            "{error:?}"
        );
    }

    #[test]
    fn rejects_ambiguous_thread_sandbox_and_permission_profile() {
        let error = resolve_thread_context(
            Some(SandboxMode::ReadOnly),
            Some(json!({ "type": "disabled" })),
            Path::new("/repo"),
            "thread/start",
        )
        .expect_err("ambiguous thread sandbox");

        assert!(
            error
                .public_message()
                .contains("choose either sandbox or permissionProfile"),
            "{error:?}"
        );
    }
}
