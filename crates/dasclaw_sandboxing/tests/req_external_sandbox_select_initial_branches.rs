//! B6-1a — ExternalSandbox `select_initial` 跨平台分支 mock 单测
//!
//! Issue: [xClaw#475](https://github.com/Linnanli/xClaw/issues/475)
//! 关联：[#473 (B6-1b)](https://github.com/Linnanli/xClaw/pull/473) 已在 Linux 容器内
//! 端到端覆盖 ExternalSandbox 的 `Auto + has_managed_network_requirements=false`
//! 分支；本文件补 `select_initial` 决策树上仍未覆盖的真实分支。
//!
//! ## 决策树（读 `manager::select_initial` + `policy_transforms::should_require_platform_sandbox` 得出）
//!
//! ```text
//! SandboxablePreference::Forbid                         → None         (契约 1)
//! SandboxablePreference::Require                        → get_platform_sandbox(level != Disabled)
//!                                                            macOS  → MacosSeatbelt           (契约 2a)
//!                                                            Linux  → LinuxSeccomp            (契约 2b)
//!                                                            Win+lvl=Disabled        → None
//!                                                            Win+lvl=RestrictedToken → WindowsRestrictedToken   (契约 2c)
//! SandboxablePreference::Auto + has_managed_network=true
//!                                                       → 同 Require 的 get_platform_sandbox  (契约 3)
//! SandboxablePreference::Auto + has_managed_network=false + ExternalSandbox FS
//!                                                       → None  (← B6-1b 已覆盖, 本文件不重测)
//! ```
//!
//! ## 为什么放在独立 `tests/` 文件
//!
//! - `crates/dasclaw_sandboxing/src/*.rs` 是 verbatim port，受
//!   `scripts/check_codex_sandboxing_drift.py` 字节级守护，不能加测试。
//! - B6-1b 文件层 `#![cfg(target_os = "linux")]` 是为 docker workflow 守门；
//!   本文件目标是跨平台 mock，与 B6-1b 不同语义，故独立成文件。
//! - 不复制 B6-1b 已覆盖的分支：那会成为补丁式 copy；本文件只覆盖真实新分支。
//!
//! ## 为什么纯 mock、零 spawn
//!
//! - 端到端 falsifier（spawn `/bin/true`）已由 B6-1b 在 `--network none
//!   --read-only` 容器里权威验证。
//! - 本文件聚焦 `select_initial` 决策函数的输入→输出契约，纯逻辑层，
//!   跨平台跑得动且无外部依赖。

use dasclaw_protocol::config_types::WindowsSandboxLevel;
use dasclaw_protocol::permissions::FileSystemSandboxPolicy;
use dasclaw_protocol::permissions::NetworkSandboxPolicy;
use dasclaw_sandboxing::SandboxManager;
use dasclaw_sandboxing::SandboxType;
use dasclaw_sandboxing::SandboxablePreference;
use dasclaw_sandboxing::get_platform_sandbox;

/// 契约 1：`Forbid` 是绝对优先级，不论 FS policy / network / managed network /
/// windows level 如何，永远返回 `None`。
///
/// 不变量：用户显式禁止后，ExternalSandbox 场景下也绝不会偷偷启平台沙箱。
#[test]
fn req_dasclaw_sandbox_b6_1a_forbid_external_sandbox_returns_none_regardless_of_other_inputs() {
    let manager = SandboxManager::new();
    let fs = FileSystemSandboxPolicy::external_sandbox();

    // 跨多组对抗性输入：managed network 强制位、网络策略、windows level
    // 全部翻转，结果必须仍是 None。
    let adversarial_inputs = [
        (
            NetworkSandboxPolicy::Restricted,
            WindowsSandboxLevel::Disabled,
            false,
        ),
        (
            NetworkSandboxPolicy::Enabled,
            WindowsSandboxLevel::Disabled,
            true,
        ),
        (
            NetworkSandboxPolicy::Restricted,
            WindowsSandboxLevel::RestrictedToken,
            true,
        ),
    ];

    for (net, win_level, managed_net) in adversarial_inputs {
        let got = manager.select_initial(
            &fs,
            net,
            SandboxablePreference::Forbid,
            win_level,
            managed_net,
        );
        assert_eq!(
            got,
            SandboxType::None,
            "Forbid 必须无视所有其它输入返回 None；\
             net={net:?}, win_level={win_level:?}, managed_network={managed_net} \
             却得到 {got:?}——这会让 ExternalSandbox 嵌套场景在用户明确禁止时\
             被偷偷启平台沙箱，违反用户意图。"
        );
    }
}

/// 契约 2：`Require` 是用户显式覆盖 ExternalSandbox 豁免逻辑的入口。
/// 即使 FS policy 是 ExternalSandbox（语义"外层已隔离"），Require 也强制
/// 返回当前 host 的平台沙箱（或 Windows 上 level=Disabled 时的 None）。
///
/// 不变量：B6-1b 的"ExternalSandbox 跳过内层"豁免**只在 Auto 路径生效**，
/// Require 路径绝不豁免。任何让 Require 在 ExternalSandbox 下也返回 None 的
/// 改动都属于回归。
#[test]
fn req_dasclaw_sandbox_b6_1a_require_external_sandbox_does_not_get_excused_by_external_signal() {
    let manager = SandboxManager::new();
    let fs = FileSystemSandboxPolicy::external_sandbox();

    let got = manager.select_initial(
        &fs,
        NetworkSandboxPolicy::Enabled,
        SandboxablePreference::Require,
        WindowsSandboxLevel::Disabled,
        /* has_managed_network_requirements */ false,
    );

    // 期望 = 当前 host 的 get_platform_sandbox(windows_sandbox_enabled = false)。
    // 不在测试里 cfg-fork 来硬编码每个 host 的预期；直接调用同一公开 API，
    // 测试关心的是"select_initial 是否调到 get_platform_sandbox 路径"——
    // 这是真正的 select_initial 决策契约，不是 get_platform_sandbox 平台映射。
    let expected = get_platform_sandbox(false).unwrap_or(SandboxType::None);

    assert_eq!(
        got, expected,
        "Require + ExternalSandbox 必须走 get_platform_sandbox 路径，\
         得到当前 host 的等价值 ({expected:?})；实际 {got:?}。\
         任何让 Require 路径偷偷豁免 ExternalSandbox 的改动都是回归。"
    );
}

/// 契约 3：`Auto + has_managed_network_requirements=true` 强制要求平台沙箱，
/// **即使 FS policy 是 ExternalSandbox**——managed network 是 hard override。
///
/// 这是 B6-1b 契约 1（`has_managed_network_requirements=false`）的对偶面。
/// 不变量：当上层（如 ManagedNetworkProxy）声明"需要 managed network"，
/// ExternalSandbox 的外层隔离假设不足以满足管控要求，必须叠加平台沙箱。
#[test]
fn req_dasclaw_sandbox_b6_1a_auto_managed_network_overrides_external_sandbox_excuse() {
    let manager = SandboxManager::new();
    let fs = FileSystemSandboxPolicy::external_sandbox();

    let got = manager.select_initial(
        &fs,
        NetworkSandboxPolicy::Restricted,
        SandboxablePreference::Auto,
        WindowsSandboxLevel::Disabled,
        /* has_managed_network_requirements */ true,
    );

    let expected = get_platform_sandbox(false).unwrap_or(SandboxType::None);

    assert_eq!(
        got, expected,
        "Auto + ExternalSandbox + managed_network=true 必须升级到平台沙箱 \
         ({expected:?})；实际 {got:?}。\
         B6-1b 契约 1 的 None 结论依赖 managed_network=false 这个必要前提；\
         本契约证明该前提一旦翻转、ExternalSandbox 的豁免立即失效。"
    );
}

/// 契约 4：Windows 特化——`WindowsSandboxLevel` 的开关位决定 Windows host 上
/// `Require` / `Auto+managed_network` 路径能否拿到 `WindowsRestrictedToken`。
///
/// 这条测试**断言相对差异**而不是绝对值：
///   `select_initial(level=Disabled)` 与 `select_initial(level=Strict)`
/// 在非 ExternalSandbox 豁免的路径（这里用 Require）上的结果差，必须等于
/// `get_platform_sandbox(false) → get_platform_sandbox(true)` 的差。
///
/// 不变量：select_initial 把 `windows_sandbox_level != Disabled` 这个 bit 透传
/// 到 `get_platform_sandbox`，不做平台特例改写。
///
/// 这条契约即使在 macOS / Linux host 上跑也有意义——非 Windows host 上
/// get_platform_sandbox 完全忽略 level bit，两次调用必然相等，本测试也据此
/// 断言 select_initial 没引入额外的平台特例。
#[test]
fn req_dasclaw_sandbox_b6_1a_windows_sandbox_level_bit_is_passed_through_to_platform_picker() {
    let manager = SandboxManager::new();
    let fs = FileSystemSandboxPolicy::external_sandbox();

    let got_disabled = manager.select_initial(
        &fs,
        NetworkSandboxPolicy::Enabled,
        SandboxablePreference::Require,
        WindowsSandboxLevel::Disabled,
        false,
    );
    let got_enabled = manager.select_initial(
        &fs,
        NetworkSandboxPolicy::Enabled,
        SandboxablePreference::Require,
        WindowsSandboxLevel::RestrictedToken,
        false,
    );

    let expected_disabled = get_platform_sandbox(false).unwrap_or(SandboxType::None);
    let expected_enabled = get_platform_sandbox(true).unwrap_or(SandboxType::None);

    assert_eq!(
        got_disabled, expected_disabled,
        "Require + ExternalSandbox + level=Disabled 必须等于 \
         get_platform_sandbox(false) 的值 ({expected_disabled:?})；实际 {got_disabled:?}。"
    );
    assert_eq!(
        got_enabled, expected_enabled,
        "Require + ExternalSandbox + level=RestrictedToken 必须等于 \
         get_platform_sandbox(true) 的值 ({expected_enabled:?})；实际 {got_enabled:?}。\
         在 Windows host 上这条断言 falsify 的是 `WindowsRestrictedToken` 与 None 的差；\
         在 macOS/Linux host 上两次调用相等也是有效信号——证明 select_initial \
         没引入非 Windows 平台的额外特例改写。"
    );
}
