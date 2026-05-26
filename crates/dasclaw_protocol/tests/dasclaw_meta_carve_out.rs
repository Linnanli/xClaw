//! Integration tests for the dasclaw `.dasclaw` meta-directory carve-out.
//!
//! ADR-136 amendment 3 (#874) extends the codex `.codex` kernel-layer
//! read-only carve-out with a symmetric `.dasclaw` entry so dasclaw-native
//! project metadata is protected with the same semantics. These tests live
//! outside `src/permissions.rs` because that file is byte-equal to its
//! upstream peer (verified by `scripts/check_codex_protocol_drift.py`) and
//! cannot host non-upstream test logic.
//!
//! The carve-out has two distinct effects that this file covers separately:
//!
//! 1. `FileSystemSandboxPolicy::from_legacy_sandbox_policy*` injects a
//!    `project_roots(".dasclaw")` entry alongside `.git`, `.agents`, and
//!    `.codex` when bridging a legacy `SandboxPolicy::WorkspaceWrite`.
//! 2. `get_writable_roots_with_cwd` projects `<root>/.dasclaw` into each
//!    writable root's `read_only_subpaths`, proactively protecting the
//!    directory even before it exists (mirroring `.codex` semantics).

use std::path::Path;

use dasclaw_absolute_path::AbsolutePathBuf;
use dasclaw_protocol::permissions::FileSystemAccessMode;
use dasclaw_protocol::permissions::FileSystemPath;
use dasclaw_protocol::permissions::FileSystemSandboxEntry;
use dasclaw_protocol::permissions::FileSystemSandboxPolicy;
use dasclaw_protocol::permissions::FileSystemSpecialPath;
use dasclaw_protocol::protocol::SandboxPolicy;
use tempfile::TempDir;

fn workspace_write_policy() -> SandboxPolicy {
    SandboxPolicy::WorkspaceWrite {
        writable_roots: Vec::new(),
        network_access: false,
        exclude_tmpdir_env_var: true,
        exclude_slash_tmp: true,
    }
}

fn has_project_root_subpath(policy: &FileSystemSandboxPolicy, name: &str) -> bool {
    policy.entries.iter().any(|entry| {
        entry.access == FileSystemAccessMode::Read
            && matches!(
                &entry.path,
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::ProjectRoots { subpath: Some(p) },
                } if p == Path::new(name),
            )
    })
}

#[test]
fn legacy_workspace_write_bridge_injects_dasclaw_alongside_codex() {
    let policy = FileSystemSandboxPolicy::from_legacy_sandbox_policy(&workspace_write_policy());

    for expected in [".git", ".agents", ".codex", ".dasclaw"] {
        assert!(
            has_project_root_subpath(&policy, expected),
            "legacy WorkspaceWrite bridge missing project-root carve-out for {expected}: \
             {policy:#?}",
        );
    }
}

#[test]
fn legacy_workspace_write_bridge_emits_dasclaw_after_codex() {
    let policy = FileSystemSandboxPolicy::from_legacy_sandbox_policy(&workspace_write_policy());

    let position = |name: &str| -> usize {
        policy
            .entries
            .iter()
            .position(|entry| {
                matches!(
                    &entry.path,
                    FileSystemPath::Special {
                        value: FileSystemSpecialPath::ProjectRoots { subpath: Some(p) },
                    } if p == Path::new(name),
                )
            })
            .unwrap_or_else(|| panic!("missing project-root entry for {name}: {policy:#?}"))
    };

    // `.dasclaw` must follow `.codex` so the upstream-equivalent prefix of the
    // entry vector stays byte-identical to codex. If this ordering ever flips
    // it most likely means the marker block in permissions.rs was moved.
    assert!(
        position(".dasclaw") > position(".codex"),
        "expected `.dasclaw` to follow `.codex` in the entry list: {policy:#?}",
    );
}

#[test]
fn writable_roots_proactively_protect_missing_dot_dasclaw() {
    let cwd = TempDir::new().expect("tempdir");
    let canonical_cwd =
        AbsolutePathBuf::from_absolute_path(cwd.path().canonicalize().expect("canonicalize cwd"))
            .expect("absolute canonical root");
    let expected_dot_dasclaw = canonical_cwd.join(".dasclaw");

    // Build a minimal restricted policy that grants write access to the
    // current working directory, mirroring the codex inline test for `.codex`.
    let policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
        path: FileSystemPath::Special {
            value: FileSystemSpecialPath::CurrentWorkingDirectory,
        },
        access: FileSystemAccessMode::Write,
    }]);

    let writable_roots = policy.get_writable_roots_with_cwd(cwd.path());

    assert_eq!(writable_roots.len(), 1, "expected single writable root");
    assert_eq!(writable_roots[0].root, canonical_cwd);
    assert!(
        writable_roots[0]
            .read_only_subpaths
            .contains(&expected_dot_dasclaw),
        "expected `.dasclaw` in read_only_subpaths, got {:#?}",
        writable_roots[0].read_only_subpaths,
    );
}

#[test]
fn cannot_write_inside_dot_dasclaw_under_workspace_write() {
    let cwd = TempDir::new().expect("tempdir");

    let file_system_policy = FileSystemSandboxPolicy::from_legacy_sandbox_policy_for_cwd(
        &workspace_write_policy(),
        cwd.path(),
    );

    assert!(
        !file_system_policy.can_write_path_with_cwd(Path::new(".dasclaw/config.toml"), cwd.path()),
        "writes inside `.dasclaw/` must be denied under WorkspaceWrite",
    );
}
