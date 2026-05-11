//! ADR-135 §3 PR-C1 / Wave-C1 contract test —— 内核层（sandbox-exec / sbpl）
//! 在 `WorkspaceWrite` 策略下必须把 `<writable_root>/.git/hooks/` 视为只读，
//! 即使该子路径在父目录的可写范围内。
//!
//! 这是 ADR-142 "WritableRoot 洞中洞" 的真实强制点。在 Wave-C1 之前，
//! `dasclaw_sandbox` 自己拼 sbpl 时不表达 `read_only_subpaths`，所以本测
//! 试在那个时间点不可能通过。委托给 `dasclaw_sandboxing::seatbelt` 之后，
//! `FileSystemSandboxPolicy::from_legacy_sandbox_policy_for_cwd` 自动注
//! 入 `.git`、`.dasclaw`、`.codex` 等保护，sandbox-exec 拒绝写入。
//!
//! ## 平台
//!
//! 仅 macOS。Linux landlock 洞中洞由 Wave-C3 落地（需要外部
//! `dasclaw-linux-sandbox` binary），Windows ACL DENY 由 ADR-141 enterprise
//! 路径提供。

#![cfg(target_os = "macos")]

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use dasclaw_sandbox::{
    Sandbox, SandboxBackendConfig, SandboxExecRequest, SandboxablePreference, SeatbeltSandbox,
};

/// Create a temporary git-like workspace and return `(root, hook_path)`.
fn make_workspace() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let hooks_dir = tmp.path().join(".git").join("hooks");
    fs::create_dir_all(&hooks_dir).expect("mkdir -p .git/hooks");
    let hook = hooks_dir.join("pre-commit");
    fs::write(&hook, "#!/bin/sh\necho safe\n").expect("seed hook");
    (tmp, hook)
}

#[test]
fn req_dasclaw_sandbox_kernel_blocks_git_hooks_under_workspace_write_macos() {
    let (tmp, hook) = make_workspace();
    let workspace_root = tmp.path().to_path_buf();

    let policy = SandboxBackendConfig {
        readable_roots: vec![PathBuf::from("/")],
        writable_roots: vec![workspace_root.clone()],
        allow_network: false,
        allow_spawn: true,
        proxy_loopback_ports: vec![],
        resource_limits: Default::default(),
    };

    // Attempt to overwrite the hook from inside the sandbox. With kernel-
    // layer 洞中洞 enforcement wired (Wave-C1), sandbox-exec must reject
    // the write even though `.git/hooks/` is nominally under a writable
    // root.
    let mut cmd = Command::new("/bin/sh");
    cmd.arg("-c").arg(format!(
        "echo malicious > {}",
        hook.to_string_lossy().replace('\'', "'\\''")
    ));
    cmd.current_dir(&workspace_root);

    let req = SandboxExecRequest {
        command: cmd,
        policy,
        preference: SandboxablePreference::Require,
        windows_sandbox_enabled: false,
    };
    let out = SeatbeltSandbox::new()
        .execute(req)
        .expect("seatbelt should run sh");

    // Fail-safe expectation: the redirect must fail at the kernel; the
    // shell exits non-zero AND the on-disk hook content remains unchanged.
    assert!(
        !out.status.success(),
        "expected sandbox-exec to block write to .git/hooks/pre-commit, but the shell exited {:?}\nstderr={}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );

    let after = fs::read_to_string(&hook).expect("read back hook");
    assert!(
        !after.contains("malicious"),
        "kernel-layer regression: .git/hooks/pre-commit was overwritten despite WorkspaceWrite policy — got: {after:?}"
    );
}

#[test]
fn req_dasclaw_sandbox_kernel_allows_write_to_non_hole_path_macos() {
    // Sibling positive contract: writes to the writable root that are NOT
    // covered by the 洞中洞 (e.g. a regular file in the workspace root)
    // must still succeed. Guards against the bridge accidentally over-
    // restricting and turning Wave-C1 into a regression.
    let tmp = tempfile::tempdir().expect("tempdir");
    let workspace_root = tmp.path().to_path_buf();
    let target = workspace_root.join("notes.txt");

    let policy = SandboxBackendConfig {
        readable_roots: vec![PathBuf::from("/")],
        writable_roots: vec![workspace_root.clone()],
        allow_network: false,
        allow_spawn: true,
        proxy_loopback_ports: vec![],
        resource_limits: Default::default(),
    };

    let mut cmd = Command::new("/bin/sh");
    cmd.arg("-c")
        .arg(format!("echo ok > {}", target.to_string_lossy()));
    cmd.current_dir(&workspace_root);

    let req = SandboxExecRequest {
        command: cmd,
        policy,
        preference: SandboxablePreference::Require,
        windows_sandbox_enabled: false,
    };
    let out = SeatbeltSandbox::new()
        .execute(req)
        .expect("seatbelt should run sh");

    assert!(
        out.status.success(),
        "expected write to {target:?} to succeed; got exit {:?}\nstderr={}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    let written = fs::read_to_string(&target).expect("read back notes.txt");
    assert_eq!(written.trim(), "ok");
}
