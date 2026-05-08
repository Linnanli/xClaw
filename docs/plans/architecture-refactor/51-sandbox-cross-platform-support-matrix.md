# 51 — Cross-Platform Sandbox Support Matrix

> **Status**: First-cut support matrix as of PR #321 (Phase 1.2)
> **Source**: 部分推进 issue #91 (`Support matrix is documented`)
> **Scope**: 跨 OS sandbox / network / resource-limit / fail-closed 行为对照
> **Related**: ADR-45 (resource limits), ADR-121 D3-3 (Windows isolation),
> ADR-129 (Windows verbatim port), 文档 49 / 50 (Windows phase 1.1 / 1.2)

## 0. 用法

本文档是**事实盘点**，**不是决策文档**。当前已知 gap（特别是 Windows
enterprise mode 的 fail-closed 表面）由 [#28 P0-A](https://github.com/Linnanli/xClaw/issues/28)
与 [#91 P1/W3](https://github.com/Linnanli/xClaw/issues/91) 两条线决定，
本文档仅记录"是什么"，不记录"应该是什么"。

## 1. SandboxType × OS 实现状态

| `SandboxType`               | macOS                           | Linux                          | Windows                                | 备注 |
|-----------------------------|---------------------------------|--------------------------------|----------------------------------------|------|
| `None`                      | ✅ direct execution             | ✅ direct execution            | ✅ direct execution                    | NoopSandbox |
| `MacosSeatbelt`             | ✅ `seatbelt::SeatbeltSandbox`  | ❌ NotImplemented              | ❌ NotImplemented                      | sandbox-exec(1) profile |
| `LinuxSeccomp`              | ❌ NotImplemented               | ✅ `linux::LinuxSeccompSandbox`| ❌ NotImplemented                      | seccomp + landlock 部分开启 |
| `WindowsRestrictedToken`    | ❌ NotImplemented               | ❌ NotImplemented              | ✅ `windows::WindowsRestrictedTokenSandbox`（PR #321 起） | Job Object + Restricted Token + Alternate Desktop（**未**含 memory/cpu cap，详见 §3） |

source: [crates/dasclaw_sandbox/src/lib.rs](../../crates/dasclaw_sandbox/src/lib.rs)
`get_platform_sandbox` / `build_backend`。

## 2. `get_platform_sandbox(windows_sandbox_enabled)` 默认选择

| 当前 OS  | `windows_sandbox_enabled=false` | `windows_sandbox_enabled=true` |
|---------|----------------------------------|--------------------------------|
| macOS   | `Some(MacosSeatbelt)`            | `Some(MacosSeatbelt)` |
| Linux   | `Some(LinuxSeccomp)`             | `Some(LinuxSeccomp)` |
| Windows | `None` → `PlatformUnavailable`   | `Some(WindowsRestrictedToken)` |
| 其它    | `None`                           | `None` |

**关键事实**：Windows 上 sandbox 是 **opt-in**（`windows_sandbox_enabled` 默
认 `false`），`SandboxablePreference::Require` 在 Windows 上会触发
`SandboxError::PlatformUnavailable`。是否升级为 fail-closed default 由 #28
P0-A enterprise contract 决定。

## 3. Capability 矩阵（每后端实际生效的隔离能力）

| Capability                    | macOS Seatbelt | Linux Seccomp+landlock | Windows RestrictedToken |
|-------------------------------|:--------------:|:----------------------:|:-----------------------:|
| Filesystem read-only by default | ✅            | ✅ (landlock)          | ✅                      |
| `writable_roots` 白名单       | ✅              | ✅                      | ✅                       |
| `allow_network` toggle        | ✅              | ✅ (proxy loopback)    | ✅ (per-user firewall)  |
| `proxy_loopback_ports` 透传   | ✅              | ✅                      | ❌（用 firewall 表达，详见 §5） |
| Process memory cap            | ✅ `memorystatus_control` | ✅ cgroup v2 memory.max | ❌ Job Object 仅设 KILL_ON_JOB_CLOSE，未设 `JOB_OBJECT_LIMIT_PROCESS_MEMORY` |
| CPU time cap                  | ✅ `RLIMIT_CPU` | ✅ `RLIMIT_CPU`         | ❌                      |
| Active process count cap      | ❌              | ✅ `RLIMIT_NPROC`       | ❌                      |
| File descriptor cap           | ✅ `RLIMIT_NOFILE` | ✅ `RLIMIT_NOFILE`   | ❌（Windows 无对应原语） |
| Process tree kill on exit     | ✅              | ✅                      | ✅                       |
| Alternate desktop / UI isolation | n/a         | n/a                    | ⚠️ 实施可用但当前 adapter 默认 `use_private_desktop=false`（first-use UX），Phase 1.3 开启 |

Windows 端"❌"项的详细分析见 §5 与 [#34](https://github.com/Linnanli/xClaw/issues/34)。

## 4. Fail-closed / fail-open 路径

| 场景 | 当前行为 | 来源 |
|------|---------|------|
| `Require` + 不支持的 OS（含 Windows 默认） | `SandboxError::PlatformUnavailable` | [lib.rs](../../crates/dasclaw_sandbox/src/lib.rs) `select_sandbox` |
| `Require` + Windows + `windows_sandbox_enabled=true` + setup 未完成 | `SandboxError::WindowsSetupPending { detail }` | [windows/mod.rs](../../crates/dasclaw_sandbox/src/windows/mod.rs) `execute` (PR #321) |
| `Forbid`              | `SandboxType::None`，直接执行       | `select_sandbox` |
| `IfAvailable` + 不支持 | fallback 到 `SandboxType::None`，**直接执行** | `select_sandbox` |

**注意**：`IfAvailable` 在 Windows 默认配置下会**fallback 到 direct
execution**。enterprise mode 是否允许这条路径由 #28 P0-A 决定；本文档仅
记录现状，不做规范判断。

## 5. Windows 已知 gap

### 5.1 资源限制缺失 (#34)

`crates/dasclaw_sandbox_windows/src/elevated/command_runner_win.rs:121`
当前 `JOBOBJECT_EXTENDED_LIMIT_INFORMATION.LimitFlags` 只包含
`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`。`ResourceLimits.max_memory_bytes`
/ `max_processes` / `max_cpu_seconds` 在 Windows backend 上是 **silent
no-op**。修复路径受 [ADR-129 §1.3 verbatim 红线](./adr-129-sandbox-windows-windows-crate-adoption.md)
约束，详见文档 [50](./50-sandbox-windows-phase-1.2-completion-and-roadmap.md) §3.2。

### 5.2 `proxy_loopback_ports` 不透传

dasclaw 抽象的 `SandboxBackendConfig.proxy_loopback_ports` 在 macOS /
Linux 上由 backend 显式打洞，但 Windows backend 目前用 per-user
firewall 表达"网络访问 yes/no"，没有"loopback 端口白名单"概念。
adapter [windows/mod.rs](../../crates/dasclaw_sandbox/src/windows/mod.rs) 显式忽略此字段；
若 enterprise 要求 proxy-only 网络模式，需扩展 firewall 表达式。

### 5.3 Alternate Desktop 默认关闭

`WindowsRestrictedTokenSandbox` 当前 hard-code `use_private_desktop=false`
以避免首次使用的 UX 摩擦（隔离桌面会让窗口"消失"）。Phase 1.3 hardening
计划提供 config toggle。

### 5.4 enterprise mode 行为未定（#91 / #28）

`require_on_unsupported_returns_unavailable` 测试已存在，但 enterprise
模式的 fail-closed contract 尚未在 Windows 上有 trip-wire 测试。本项是
#91 第三项 acceptance（"Unsupported behavior disables shell or fails
closed before execution"），强依赖 #28 P0-A。

## 6. 测试覆盖

| 平台 | 单元测试 | 集成测试 |
|------|---------|---------|
| macOS Seatbelt | ✅ `dasclaw_sandbox::macos::tests::*` | ⚠️ host-gated |
| Linux Seccomp | ✅ `dasclaw_sandbox::linux::tests::*` | ⚠️ host-gated |
| Windows RestrictedToken (adapter) | ✅ `dasclaw_sandbox::windows::tests::*` (cfg(target_os = "windows")) | ⚠️ 仅 Windows runner，PR #322 起由 path filter 自动触发 |
| 跨平台抽象 | ✅ `dasclaw_sandbox::tests::*`（含 `require_on_unsupported_returns_unavailable`） | n/a |

注：本文档的"✅" 仅表示存在测试代码，**不**表示行为对齐——是否对齐由
#91 / #28 的 enterprise contract 决定。

## 7. 不在本文档范围

- ❌ Sandbox 行为应该如何（normative 规范）
- ❌ enterprise fail-closed contract（属 #28 P0-A）
- ❌ Windows resource-limit 实施路径决议（属 #34 + Phase 1.3 ADR）
- ❌ MCP / WASM / extension sandbox（属 #96 P0-J）
