// Derived from openai/codex commit 6e838a19fa
//   path: codex-rs/protocol/src/protocol.rs (SandboxPolicy / NetworkAccess / WritableRoot)
// SPDX-License-Identifier: Apache-2.0

//! Sandbox policy types, inlined from the upstream `codex-protocol` crate.
//!
//! The upstream definitions additionally derive `strum::Display`, `JsonSchema`,
//! and `TS`. Those derives drag in `strum_macros`, `schemars`, and `ts-rs`,
//! which we do not yet vendor. They are intentionally omitted here in
//! Phase 1.1.0; downstream PRs may reintroduce them if x-claw needs the
//! corresponding capabilities.
//!
//! `AbsolutePathBuf` is also part of the upstream sandbox-policy surface; it
//! lives in `codex-utils-absolute-path`. Until that helper is ported in
//! PR-1.1.1, this module uses `std::path::PathBuf` for `writable_roots`. The
//! serde representation is unchanged (a JSON array of path strings), so the
//! switch to `AbsolutePathBuf` is a non-breaking refinement.

use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

/// Represents whether outbound network access is available to the agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum NetworkAccess {
    #[default]
    Restricted,
    Enabled,
}

impl NetworkAccess {
    pub fn is_enabled(self) -> bool {
        matches!(self, NetworkAccess::Enabled)
    }
}

/// Determines execution restrictions for model shell commands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum SandboxPolicy {
    /// No restrictions whatsoever. Use with caution.
    #[serde(rename = "danger-full-access")]
    DangerFullAccess,

    /// Read-only access configuration.
    #[serde(rename = "read-only")]
    ReadOnly {
        /// When set to `true`, outbound network access is allowed. `false` by
        /// default.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        network_access: bool,
    },

    /// Indicates the process is already in an external sandbox. Allows full
    /// disk access while honoring the provided network setting.
    #[serde(rename = "external-sandbox")]
    ExternalSandbox {
        /// Whether the external sandbox permits outbound network traffic.
        #[serde(default)]
        network_access: NetworkAccess,
    },

    /// Same as `ReadOnly` but additionally grants write access to the current
    /// working directory ("workspace").
    #[serde(rename = "workspace-write")]
    WorkspaceWrite {
        /// Additional folders (beyond cwd and possibly TMPDIR) that should be
        /// writable from within the sandbox.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        writable_roots: Vec<PathBuf>,

        /// When set to `true`, outbound network access is allowed. `false` by
        /// default.
        #[serde(default)]
        network_access: bool,

        /// When set to `true`, will NOT include the per-user `TMPDIR`
        /// environment variable among the default writable roots. Defaults to
        /// `false`.
        #[serde(default)]
        exclude_tmpdir_env_var: bool,

        /// When set to `true`, will NOT include the `/tmp` among the default
        /// writable roots on UNIX. Defaults to `false`.
        #[serde(default)]
        exclude_slash_tmp: bool,
    },
}

/// A writable root path accompanied by a list of subpaths that should remain
/// read-only even when the root is writable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WritableRoot {
    pub root: PathBuf,

    /// By construction, these subpaths are all under `root`.
    pub read_only_subpaths: Vec<PathBuf>,
}

impl WritableRoot {
    pub fn is_path_writable(&self, path: &Path) -> bool {
        if !path.starts_with(&self.root) {
            return false;
        }
        for subpath in &self.read_only_subpaths {
            if path.starts_with(subpath) {
                return false;
            }
        }
        true
    }
}

impl SandboxPolicy {
    /// Returns a policy with read-only disk access and no network.
    pub fn new_read_only_policy() -> Self {
        SandboxPolicy::ReadOnly {
            network_access: false,
        }
    }

    /// Returns a policy that can read the entire disk, but can only write to
    /// the current working directory and the per-user tmp dir on macOS. It
    /// does not allow network access.
    pub fn new_workspace_write_policy() -> Self {
        SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![],
            network_access: false,
            exclude_tmpdir_env_var: false,
            exclude_slash_tmp: false,
        }
    }

    pub fn has_full_disk_read_access(&self) -> bool {
        true
    }

    pub fn has_full_disk_write_access(&self) -> bool {
        match self {
            SandboxPolicy::DangerFullAccess => true,
            SandboxPolicy::ExternalSandbox { .. } => true,
            SandboxPolicy::ReadOnly { .. } => false,
            SandboxPolicy::WorkspaceWrite { .. } => false,
        }
    }

    pub fn has_full_network_access(&self) -> bool {
        match self {
            SandboxPolicy::DangerFullAccess => true,
            SandboxPolicy::ExternalSandbox { network_access } => network_access.is_enabled(),
            SandboxPolicy::ReadOnly { network_access, .. } => *network_access,
            SandboxPolicy::WorkspaceWrite { network_access, .. } => *network_access,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn read_only_policy_round_trips_through_serde_json() {
        let policy = SandboxPolicy::new_read_only_policy();
        let json = serde_json::to_string(&policy).expect("serialize");
        let parsed: SandboxPolicy = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(policy, parsed);
    }

    #[test]
    fn workspace_write_policy_default_excludes_optional_fields() {
        let policy = SandboxPolicy::new_workspace_write_policy();
        let json = serde_json::to_string(&policy).expect("serialize");
        // network_access serializes (because it is `false` by default but not
        // skipped); writable_roots is empty so `skip_serializing_if` drops it.
        assert!(json.contains("\"type\":\"workspace-write\""));
        assert!(!json.contains("writable_roots"));
        assert!(!policy.has_full_disk_write_access());
        assert!(!policy.has_full_network_access());
    }

    #[test]
    fn external_sandbox_with_network_enabled_round_trips() {
        let policy = SandboxPolicy::ExternalSandbox {
            network_access: NetworkAccess::Enabled,
        };
        assert!(policy.has_full_disk_write_access());
        assert!(policy.has_full_network_access());

        let json = serde_json::to_string(&policy).expect("serialize");
        let parsed: SandboxPolicy = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(policy, parsed);
    }

    #[test]
    fn writable_root_blocks_paths_under_read_only_subpath() {
        let root = WritableRoot {
            root: PathBuf::from("/repo"),
            read_only_subpaths: vec![PathBuf::from("/repo/.git")],
        };
        assert!(root.is_path_writable(Path::new("/repo/src/main.rs")));
        assert!(!root.is_path_writable(Path::new("/repo/.git/HEAD")));
        assert!(!root.is_path_writable(Path::new("/other/path")));
    }
}
