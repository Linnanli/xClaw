//! ADR-141 §6 enterprise fail-closed gate — pure-function 决策矩阵契约测试。
//!
//! `check_enterprise_gate` 是 ADR-141 PR-W1 引入的跨平台 pure decision
//! function：所有三个 OS sandbox 后端（Windows / macOS / Linux）在 `execute`
//! 入口先跑它，把 enterprise 模式下"kernel 层 carve-out enforcement 缺失"
//! 这一类风险**统一拒绝**在 IPC / 进程派发之前。
//!
//! 本契约测试覆盖 ADR-141 §6 OQ-1（default `false` / fail-closed）+
//! OQ-W3-3（gate signal 用 `read_only_subpaths.is_empty()` 而不是
//! `writable_roots.is_empty()`）+ Wave-C1c P1.2b sign-off（删 soft-mode
//! 后只剩 `Allow` / `DenyNoKernelSandbox` 两个 outcome）。
//!
//! ## 为什么是 host-agnostic 而不是 `#[cfg(windows)]`
//!
//! `check_enterprise_gate` 故意设计为 **side-effect-free pure function**，
//! 全决策矩阵在 macOS / Linux CI runner 上即可完整测——不需要 Windows
//! VM 也能守 ADR-141 §6 fail-closed 契约。这就是 epic #380 Batch 3 B3-3
//! 列三个测试名带 `windows_enterprise` 但实际跨平台跑的原因：测的是 OQ
//! 决策，不是 Windows IPC。

use std::path::PathBuf;

use dasclaw_sandbox::{
    check_enterprise_gate, EnterpriseGateOutcome, SandboxBackendConfig, SandboxType,
};

/// 构造一个 enterprise=on + Windows + setup_complete=true 的基础 config，
/// 各测试通过 mutator 改单一字段以隔离决策分支。
fn enterprise_windows_config() -> SandboxBackendConfig {
    SandboxBackendConfig {
        enterprise_mode: true,
        ..SandboxBackendConfig::default()
    }
}

#[test]
fn req_dasclaw_exec_windows_enterprise_allow_when_enterprise_off() {
    // 兜底：enterprise_mode=false 时无论 sandbox_type / setup_complete 取何值，
    // gate 必须 Allow（默认非企业场景行为不变）。
    let config = SandboxBackendConfig {
        enterprise_mode: false,
        read_only_subpaths: vec![PathBuf::from(".git/hooks")],
        ..SandboxBackendConfig::default()
    };

    let outcome = check_enterprise_gate(
        &config,
        SandboxType::None, // 即使 sandbox 不存在
        false,             // 即使 setup 未完成
    );

    assert_eq!(
        outcome,
        EnterpriseGateOutcome::Allow,
        "non-enterprise spawn must short-circuit to Allow regardless of \
         sandbox availability — see ADR-141 §6 OQ-1 default `false`"
    );
}

#[test]
fn req_dasclaw_exec_windows_enterprise_fail_closed_when_no_sandbox() {
    // ADR-141 §6 OQ-1：enterprise_mode=true 但 sandbox_type=None
    // （平台无 kernel sandbox 后端可用）必须 fail-closed，
    // 调用方据此返回 SandboxError::WindowsSandboxNotAvailable。
    let config = enterprise_windows_config();

    let outcome = check_enterprise_gate(
        &config,
        SandboxType::None,
        true, // setup_complete 不影响 Step 1 的 None 判定
    );

    assert_eq!(
        outcome,
        EnterpriseGateOutcome::DenyNoKernelSandbox,
        "enterprise spawn must be denied when no kernel sandbox backend \
         exists on host — caller surfaces SandboxError::WindowsSandboxNotAvailable"
    );
}

#[test]
fn req_dasclaw_exec_windows_enterprise_fail_closed_when_kernel_setup_incomplete() {
    // ADR-141 §3 PR-W1：sandbox 后端类型已选定（WindowsRestrictedToken），
    // 但平台 setup 尚未完成（sandbox_setup_is_complete = false）→ 必须
    // fail-closed。守 "首次部署未跑 dasclaw-sandbox-setup.exe 就被静默
    // 降级" 这条 OWASP A05 安全配置失误。
    let config = enterprise_windows_config();

    let outcome = check_enterprise_gate(
        &config,
        SandboxType::WindowsRestrictedToken,
        false, // kernel_setup_complete=false（Windows: IT 还没跑 elevated setup）
    );

    assert_eq!(
        outcome,
        EnterpriseGateOutcome::DenyNoKernelSandbox,
        "enterprise spawn must be denied when sandbox infra is selected \
         but setup is incomplete — fail-closed audit trail per ADR-141 §3 PR-W1"
    );
}

#[test]
fn test_security_sandbox_no_bypass() {
    let mut config = enterprise_windows_config();
    config
        .read_only_subpaths
        .push(PathBuf::from("/workspace/.git/hooks"));

    for (sandbox_type, setup_complete) in [
        (SandboxType::None, true),
        (SandboxType::WindowsRestrictedToken, false),
        (SandboxType::MacosSeatbelt, false),
        (SandboxType::LinuxSeccomp, false),
    ] {
        let outcome = check_enterprise_gate(&config, sandbox_type, setup_complete);
        assert_eq!(
            outcome,
            EnterpriseGateOutcome::DenyNoKernelSandbox,
            "enterprise carve-out enforcement must fail closed before process dispatch for {sandbox_type:?}"
        );
    }
}

#[test]
fn req_dasclaw_exec_windows_enterprise_allow_when_no_carveouts() {
    // OQ-W3-3 sign-off 2026-05-11+：原 PR-W1+W2 用
    // `writable_roots.is_empty()` 做 gate signal 是错的——WorkspaceWrite
    // policy 总是带 writable_roots，会把"无洞中洞的常见任务"误判为
    // "需要 kernel enforcement"。修正后用 `read_only_subpaths.is_empty()`：
    // 没有 carve-out 时 kernel 层根本无须额外 deny，base sandbox 就够了。
    let mut config = enterprise_windows_config();
    config.writable_roots.push(PathBuf::from("/workspace"));
    // 注意：故意保持 read_only_subpaths 为空。

    let outcome = check_enterprise_gate(&config, SandboxType::WindowsRestrictedToken, true);

    assert_eq!(
        outcome,
        EnterpriseGateOutcome::Allow,
        "WorkspaceWrite without holes must short-circuit to Allow — \
         OQ-W3-3 fix prevents misclassifying the common case as \
         'needs kernel carve-out enforcement'"
    );
}

#[test]
fn req_dasclaw_exec_windows_enterprise_allow_when_kernel_carveouts_wired() {
    // Wave-C1c P1.2b sign-off：三平台 OS sandbox 后端（macOS Seatbelt /
    // Windows RestrictedToken / Linux Seccomp）的 kernel 层 carve-out
    // enforcement 都已落地（分别由 Wave-C1a / Wave-C1b PR #426 / Wave-C1c
    // PR #444+#445+#448 闭环），所以 enterprise + carve-outs + 完整 setup
    // → 仍是 Allow（让具体后端去落实 DACL/sbpl/landlock）。
    let mut config = enterprise_windows_config();
    config
        .read_only_subpaths
        .push(PathBuf::from("/workspace/.git/hooks"));

    for kind in [
        SandboxType::WindowsRestrictedToken,
        SandboxType::MacosSeatbelt,
        SandboxType::LinuxSeccomp,
    ] {
        let outcome = check_enterprise_gate(&config, kind, true);
        assert_eq!(
            outcome,
            EnterpriseGateOutcome::Allow,
            "enterprise + carve-outs + kernel ready → Allow for {kind:?}; \
             see ADR-141 §1.1 matrix + ADR-144 §5.3 (Linux Wave-C1c)"
        );
    }
}
