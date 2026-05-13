//! B6-1b — ExternalSandbox 嵌套契约（必须在 docker 容器内跑才有意义）
//!
//! Issue：[xClaw#472](https://github.com/Linnanli/xClaw/issues/472)
//! 范围：epic [#380](https://github.com/Linnanli/xClaw/issues/380) Batch 6
//! 关联：[`docs/plans/architecture-refactor/32-execution-plan.md`](../../../../docs/plans/architecture-refactor/32-execution-plan.md)
//! §W2 验收"ExternalSandbox 嵌套测试"
//!
//! ## 测什么
//!
//! 当用户在外层 docker / firecracker / nsjail 里跑 xClaw，并显式声明
//! `SandboxPolicy::ExternalSandbox { network_access: ... }`，本模块验证：
//!
//! 1. **`SandboxManager::select_initial` 在 ExternalSandbox 的 FS policy
//!    （unrestricted、无 managed network requirements）下返回
//!    `SandboxType::None`**——不会去启 bubblewrap / landlock / seatbelt /
//!    Windows Job Object，因为外层容器已是隔离边界。
//!
//! 2. **`transform()` 返回的 argv 是用户程序本身**，没有 `bwrap` /
//!    `sandbox-exec` / `dasclaw-sandbox-resource-launcher` wrapper 前缀。
//!    这是在容器里跑时不会因为缺 bubblewrap / 缺 CAP_SYS_ADMIN 而炸的关键。
//!
//! 3. **`network_access` 字段照常透传**：`Restricted` 不会被偷偷升级，
//!    `Enabled` 也不会被偷偷降级——容器编排者依赖这个字段决定是否启
//!    docker `--network none`，逻辑不能漂移。
//!
//! 4. **真实 spawn `/bin/true` 在 ExternalSandbox + None 模式下退出 0**：
//!    这条断言只有在 *没有* OS sandbox 包装时才能在 `--network none
//!    --read-only` debian 容器里成功——是嵌套契约的端到端 falsifier。
//!
//! ## 为什么不放 `manager_tests.rs`
//!
//! `crates/dasclaw_sandboxing/src/*.rs` 是 verbatim port，受
//! `scripts/check_codex_sandboxing_drift.py` 守护，**不能加测试**。
//! 集成测试在 `tests/` 目录，drift guard 不约束。
//!
//! ## 为什么 `#[cfg(target_os = "linux")]`
//!
//! 嵌套契约的真实落地场景是 docker（Linux 容器）。在 macOS host 上单跑
//! 这个集成测试也能过（语义跨平台），但 CI workflow
//! `external-sandbox-container.yml` 才是它的唯一权威触发点——把这个 cfg
//! 守门留给 workflow，不污染 macOS 本地开发循环。

#![cfg(target_os = "linux")]

use std::collections::HashMap;
use std::process::Command;

use dasclaw_absolute_path::AbsolutePathBuf;
use dasclaw_protocol::config_types::WindowsSandboxLevel;
use dasclaw_protocol::permissions::FileSystemSandboxPolicy;
use dasclaw_protocol::permissions::NetworkSandboxPolicy;
use dasclaw_protocol::protocol::NetworkAccess;
use dasclaw_protocol::protocol::SandboxPolicy;
use dasclaw_sandboxing::SandboxCommand;
use dasclaw_sandboxing::SandboxManager;
use dasclaw_sandboxing::SandboxTransformRequest;
use dasclaw_sandboxing::SandboxType;
use dasclaw_sandboxing::SandboxablePreference;

/// 契约 1：FS policy 是 `ExternalSandbox`（语义等价 unrestricted）+
/// `SandboxablePreference::Auto` + 无 managed network 时，select_initial
/// 不应启平台沙箱。这是 codex 把 ExternalSandbox 当"外层已隔离"信号的根。
#[test]
fn req_dasclaw_sandbox_b6_1b_external_sandbox_selects_no_inner_sandbox() {
    let manager = SandboxManager::new();
    let sandbox = manager.select_initial(
        &FileSystemSandboxPolicy::external_sandbox(),
        NetworkSandboxPolicy::Enabled,
        SandboxablePreference::Auto,
        WindowsSandboxLevel::Disabled,
        /* has_managed_network_requirements */ false,
    );
    assert_eq!(
        sandbox,
        SandboxType::None,
        "ExternalSandbox 的 FS policy 在 Auto + 无 managed network 下\
         必须落到 SandboxType::None——这是嵌套场景下不再启内层平台沙箱\
         的判定点。任何回归都会让容器里的 spawn 试图调 bubblewrap 而失败。"
    );
}

/// 契约 2：transform 出的 argv = 用户程序本体，无任何 OS sandbox 前缀。
/// 这条在 `--network none --read-only` 容器内跑时尤其关键：任何 wrapper
/// 都依赖 root / CAP_SYS_ADMIN / 写入 /proc 等容器里不可得的能力。
#[test]
fn req_dasclaw_sandbox_b6_1b_external_sandbox_transform_yields_bare_argv() {
    let manager = SandboxManager::new();
    let cwd = AbsolutePathBuf::current_dir().expect("current dir");
    let exec_request = manager
        .transform(SandboxTransformRequest {
            command: SandboxCommand {
                program: "/bin/true".into(),
                args: Vec::new(),
                cwd: cwd.clone(),
                env: HashMap::new(),
                additional_permissions: None,
            },
            policy: &SandboxPolicy::ExternalSandbox {
                network_access: NetworkAccess::Restricted,
            },
            file_system_policy: &FileSystemSandboxPolicy::external_sandbox(),
            network_policy: NetworkSandboxPolicy::Restricted,
            sandbox: SandboxType::None,
            enforce_managed_network: false,
            network: None,
            sandbox_policy_cwd: cwd.as_path(),
            codex_linux_sandbox_exe: None,
            use_legacy_landlock: false,
            windows_sandbox_level: WindowsSandboxLevel::Disabled,
            windows_sandbox_private_desktop: false,
        })
        .expect("transform must succeed for ExternalSandbox + SandboxType::None");

    assert_eq!(
        exec_request.command.first().map(String::as_str),
        Some("/bin/true"),
        "argv[0] 必须仍是用户程序本身。任何 bwrap/sandbox-exec/launcher 前缀\
         都意味着内层 sandbox 试图启用——会在裸容器里直接炸。"
    );
    assert!(
        exec_request.arg0.is_none(),
        "ExternalSandbox + None 不应触发 arg0 重写。"
    );
}

/// 契约 3：network_access 字段在 ExternalSandbox 模式下不被透明改写。
/// 容器编排（docker --network none 与否）依赖这条信号准确。
#[test]
fn req_dasclaw_sandbox_b6_1b_external_sandbox_preserves_network_access_signal() {
    let restricted = SandboxPolicy::ExternalSandbox {
        network_access: NetworkAccess::Restricted,
    };
    let enabled = SandboxPolicy::ExternalSandbox {
        network_access: NetworkAccess::Enabled,
    };

    assert!(
        !restricted.has_full_network_access(),
        "ExternalSandbox{{Restricted}} 必须报告无全量网络访问——\
         编排层据此选择 docker --network none。"
    );
    assert!(
        enabled.has_full_network_access(),
        "ExternalSandbox{{Enabled}} 必须报告有全量网络访问——\
         编排层据此放开容器网络。"
    );
}

/// 契约 4：端到端 falsifier。spawn `/bin/true` 在 ExternalSandbox 模式
/// 走 `SandboxType::None` 路径，必须真的能在裸容器里跑出 exit 0。
///
/// 这条测试在 macOS host 上也能过，但价值是被
/// `external-sandbox-container.yml` 在 `--network none --read-only`
/// 容器里跑——那才是它真正能 falsify"内层 sandbox 不会被错误启用"
/// 这条不变量的环境。
#[test]
fn req_dasclaw_sandbox_b6_1b_external_sandbox_spawn_actually_works() {
    let manager = SandboxManager::new();
    let cwd = AbsolutePathBuf::current_dir().expect("current dir");
    let exec_request = manager
        .transform(SandboxTransformRequest {
            command: SandboxCommand {
                program: "/bin/true".into(),
                args: Vec::new(),
                cwd: cwd.clone(),
                env: HashMap::new(),
                additional_permissions: None,
            },
            policy: &SandboxPolicy::ExternalSandbox {
                network_access: NetworkAccess::Restricted,
            },
            file_system_policy: &FileSystemSandboxPolicy::external_sandbox(),
            network_policy: NetworkSandboxPolicy::Restricted,
            sandbox: SandboxType::None,
            enforce_managed_network: false,
            network: None,
            sandbox_policy_cwd: cwd.as_path(),
            codex_linux_sandbox_exe: None,
            use_legacy_landlock: false,
            windows_sandbox_level: WindowsSandboxLevel::Disabled,
            windows_sandbox_private_desktop: false,
        })
        .expect("transform");

    let argv = &exec_request.command;
    let Some((program, args)) = argv.split_first() else {
        panic!("transform 返回了空 argv——契约 2 应已先拦下，回归。")
    };

    let status = Command::new(program)
        .args(args)
        .status()
        .expect("spawn /bin/true 失败——可能容器内 PATH/binary 缺失");

    assert!(
        status.success(),
        "ExternalSandbox + None 路径下 /bin/true 应退出 0；\
         实际 status = {status:?}。回归常因内层 sandbox 被错误启用、\
         在容器里因缺 CAP_SYS_ADMIN 而初始化失败导致。"
    );
}
