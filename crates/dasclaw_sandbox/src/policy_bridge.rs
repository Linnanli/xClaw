//! Backend-config → protocol-policy bridge (shared by macOS Seatbelt and
//! Linux helper wiring).
//!
//! Translates the flat backend-level [`SandboxBackendConfig`] used by
//! `dasclaw_exec`/`dasclaw_sandbox` into the high-level
//! `dasclaw_protocol::SandboxPolicy` enum + `FileSystemSandboxPolicy` +
//! `NetworkSandboxPolicy` triplet consumed by `dasclaw_sandboxing`
//! argv builders (`seatbelt::create_seatbelt_command_args` on macOS,
//! `landlock::create_linux_sandbox_command_args_for_policies` on Linux).
//!
//! ## History
//!
//! - ADR-135 §3 PR-C1 (Wave-C1a): introduced as `macos/policy_bridge.rs`
//!   to back the macOS Seatbelt rewrite.
//! - ADR-144 §5.3 P1.2a: promoted to a shared crate-level module so the
//!   Linux helper wiring can reuse the exact same translation logic.
//!
//! ## Lossy direction
//!
//! The high-level four-tier enum → flat config direction (handled by
//! `dasclaw_exec::policy_to_backend_config_with_env`) is intentionally
//! lossy. This bridge reconstructs an *equivalent* policy:
//!
//! - empty `writable_roots` ⇒ `SandboxPolicy::ReadOnly`
//! - non-empty `writable_roots` ⇒ `SandboxPolicy::WorkspaceWrite`
//!   with `exclude_tmpdir_env_var: true` + `exclude_slash_tmp: true`,
//!   because the caller (`dasclaw_exec`) has already materialised every
//!   intended writable root into `writable_roots`. Leaving the
//!   `exclude_*` flags `false` would let
//!   `FileSystemSandboxPolicy::from_legacy_*` silently append `/tmp` and
//!   `$TMPDIR` again — broadening access relative to the caller's intent.
//!
//! Network access is mirrored as a boolean. `proxy_loopback_ports` is
//! plumbed to backends via `SandboxExecRequest.network` (macOS) /
//! `req.policy.proxy_loopback_ports` (Linux env-detect path).

use std::path::Path;

use dasclaw_absolute_path::AbsolutePathBuf;
use dasclaw_protocol::permissions::FileSystemSandboxPolicy;
use dasclaw_protocol::permissions::NetworkSandboxPolicy;
use dasclaw_protocol::protocol::SandboxPolicy as ProtocolSandboxPolicy;

use crate::SandboxBackendConfig;

/// Result of bridging a [`SandboxBackendConfig`] for consumption by
/// `dasclaw_sandboxing` argv builders.
pub(crate) struct ProtocolPolicyTriple {
    pub sandbox_policy: ProtocolSandboxPolicy,
    pub file_system_policy: FileSystemSandboxPolicy,
    pub network_policy: NetworkSandboxPolicy,
}

/// Translate a flat `SandboxBackendConfig` into the upstream protocol
/// policy shapes that `dasclaw_sandboxing` consumes.
///
/// `cwd` is the directory the sandbox policy is anchored to; it is also
/// used as a fallback when a writable root path cannot be converted into an
/// `AbsolutePathBuf`. Invalid roots are dropped (rather than panicking) and
/// the resulting policy may be more restrictive than the input — fail-safe.
pub(crate) fn to_protocol_policy(
    backend: &SandboxBackendConfig,
    cwd: &Path,
) -> ProtocolPolicyTriple {
    let writable_roots: Vec<AbsolutePathBuf> = backend
        .writable_roots
        .iter()
        .filter_map(|p| AbsolutePathBuf::from_absolute_path_checked(p).ok())
        .collect();

    let sandbox_policy = if writable_roots.is_empty() {
        ProtocolSandboxPolicy::ReadOnly {
            network_access: backend.allow_network,
        }
    } else {
        ProtocolSandboxPolicy::WorkspaceWrite {
            writable_roots,
            network_access: backend.allow_network,
            // The caller has already enumerated every intended writable
            // root in `backend.writable_roots`. Suppress the protocol's
            // automatic `/tmp` + `$TMPDIR` injection so the kernel policy
            // never grants writes outside the caller's stated set.
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        }
    };

    let file_system_policy =
        FileSystemSandboxPolicy::from_legacy_sandbox_policy_for_cwd(&sandbox_policy, cwd);
    let network_policy = NetworkSandboxPolicy::from(&sandbox_policy);

    ProtocolPolicyTriple {
        sandbox_policy,
        file_system_policy,
        network_policy,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn empty_backend() -> SandboxBackendConfig {
        SandboxBackendConfig {
            readable_roots: vec![PathBuf::from("/")],
            writable_roots: vec![],
            read_only_subpaths: vec![],
            allow_network: false,
            allow_spawn: true,
            proxy_loopback_ports: vec![],
            resource_limits: Default::default(),
            enterprise_mode: false,
            enterprise_allow_userspace_carveouts: false,
            linux_sandbox_exe: None,
        }
    }

    #[test]
    fn empty_writable_roots_yields_read_only() {
        let backend = empty_backend();
        let cwd = PathBuf::from("/");
        let triple = to_protocol_policy(&backend, &cwd);
        assert!(matches!(
            triple.sandbox_policy,
            ProtocolSandboxPolicy::ReadOnly {
                network_access: false
            }
        ));
    }

    #[test]
    fn writable_roots_yield_workspace_write_with_excludes() {
        let mut backend = empty_backend();
        backend.writable_roots = vec![PathBuf::from("/tmp/work")];
        backend.allow_network = true;
        let cwd = PathBuf::from("/tmp/work");
        let triple = to_protocol_policy(&backend, &cwd);
        match triple.sandbox_policy {
            ProtocolSandboxPolicy::WorkspaceWrite {
                ref writable_roots,
                network_access,
                exclude_tmpdir_env_var,
                exclude_slash_tmp,
            } => {
                assert_eq!(writable_roots.len(), 1);
                assert!(network_access);
                assert!(exclude_tmpdir_env_var);
                assert!(exclude_slash_tmp);
            }
            other => panic!("expected WorkspaceWrite, got {other:?}"),
        }
    }

    #[test]
    fn relative_writable_root_is_dropped_fail_safe() {
        let mut backend = empty_backend();
        backend.writable_roots = vec![PathBuf::from("relative/path")];
        let cwd = PathBuf::from("/");
        let triple = to_protocol_policy(&backend, &cwd);
        // Relative path is rejected → no writable roots → ReadOnly.
        assert!(matches!(
            triple.sandbox_policy,
            ProtocolSandboxPolicy::ReadOnly { .. }
        ));
    }
}
