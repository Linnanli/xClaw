//! H2 (ADR-144 §5.3 P1.2a) — Linux kernel-enforced `read_only_subpaths` 集成测试。
//!
//! 当 `linux_sandbox_exe` 配置好 helper 时，对 `read_only_subpaths` 下的
//! 写入必须被 kernel 拒绝（EACCES / 子进程非零退出）。当 helper 未配置
//! 但 spawn 携带 carve-outs 时，必须 fail-closed 返回
//! `ReadOnlySubpathsKernelEnforcementMissing`，**不能**静默降级为
//! seccomp-only（守 ADR-141 §6 OQ-1）。
//!
//! 仅在 Linux target 启用 — 其他平台编译期完全跳过。
//!
//! ## 真实路径前置
//!
//! 测试用例 `req_dasclaw_sandbox_p1_2a_helper_blocks_read_only_subpath` 需要
//! 真实的 `dasclaw-sandbox-linux` helper binary 存在。CI 矩阵在 Linux
//! runner 上 `cargo build -p dasclaw_sandbox_linux` 后会把产物放在
//! `target/debug/dasclaw-sandbox-linux`；本测试自动探测该路径。本地未构建
//! helper 时跳过（不算失败）。

#![cfg(target_os = "linux")]

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use dasclaw_sandbox::{
    LinuxSeccompSandbox, Sandbox, SandboxBackendConfig, SandboxError, SandboxExecRequest,
    SandboxablePreference,
};

/// 探测仓库构建出的 helper 路径；找不到返回 `None`。
fn locate_helper() -> Option<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // crates/dasclaw_sandbox/ → repo_root
    let repo_root = manifest_dir.parent()?.parent()?;
    for candidate in [
        repo_root.join("target/debug/dasclaw-sandbox-linux"),
        repo_root.join("target/release/dasclaw-sandbox-linux"),
    ] {
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

#[test]
fn req_dasclaw_sandbox_p1_2a_helper_blocks_read_only_subpath() {
    let helper = match locate_helper() {
        Some(p) => p,
        None => {
            eprintln!(
                "skip: dasclaw-sandbox-linux helper not built — \
                 run `cargo build -p dasclaw_sandbox_linux` first"
            );
            return;
        }
    };

    let workspace = tempfile::tempdir().expect("tempdir");
    let workspace_root = workspace.path().to_path_buf();
    let git_hooks = workspace_root.join(".git").join("hooks");
    fs::create_dir_all(&git_hooks).expect("mkdir .git/hooks");
    let hook = git_hooks.join("pre-commit");
    fs::write(&hook, "#!/bin/sh\nexit 0\n").expect("seed hook");

    let mut policy = SandboxBackendConfig::default();
    policy.readable_roots = vec![PathBuf::from("/")];
    policy.writable_roots = vec![workspace_root.clone()];
    policy.read_only_subpaths = vec![workspace_root.join(".git/hooks")];
    policy.allow_network = false;
    policy.allow_spawn = true;
    policy.linux_sandbox_exe = Some(helper);

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
        network: None,
    };
    let out = LinuxSeccompSandbox::new()
        .execute(req)
        .expect("helper should spawn even if user command fails");

    assert!(
        !out.status.success(),
        "helper should block write to .git/hooks/pre-commit, shell exited {:?}\nstderr={}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    let after = fs::read_to_string(&hook).expect("read hook back");
    assert!(
        !after.contains("malicious"),
        "kernel-layer regression: .git/hooks/pre-commit was overwritten despite read_only_subpaths"
    );
}

#[test]
fn req_dasclaw_sandbox_p1_2a_fail_closed_when_helper_missing() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let workspace_root = workspace.path().to_path_buf();

    let mut policy = SandboxBackendConfig::default();
    policy.readable_roots = vec![PathBuf::from("/")];
    policy.writable_roots = vec![workspace_root.clone()];
    policy.read_only_subpaths = vec![workspace_root.join(".git/hooks")];
    policy.linux_sandbox_exe = None; // 未配置 helper → fail-closed

    let mut cmd = Command::new("/bin/true");
    cmd.current_dir(&workspace_root);

    let req = SandboxExecRequest {
        command: cmd,
        policy,
        preference: SandboxablePreference::Require,
        windows_sandbox_enabled: false,
        network: None,
    };
    let err = LinuxSeccompSandbox::new()
        .execute(req)
        .expect_err("must fail-closed when helper missing and carve-outs requested");

    assert!(
        matches!(
            err,
            SandboxError::ReadOnlySubpathsKernelEnforcementMissing { .. }
        ),
        "expected ReadOnlySubpathsKernelEnforcementMissing, got {err:?}"
    );
}
