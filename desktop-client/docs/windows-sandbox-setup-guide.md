# Dasclaw Windows Sandbox — 政企部署指南

> **目标读者**：企业 IT 管理员 / SCCM / Intune / GPO 运维。
>
> **决策依据**：
> - [ADR-121](../../docs/plans/architecture-refactor/adr-121-p0a-sandbox-activation-decision.md) — 沙箱激活默认 fail-closed
> - [ADR-129](../../docs/plans/architecture-refactor/adr-129-sandbox-windows-windows-crate-adoption.md) — verbatim 移植红线
> - [ADR-130](../../docs/plans/architecture-refactor/adr-130-sandbox-windows-lib-bin-split.md) — lib/bin 拆分
> - [ADR-141](../../docs/plans/architecture-refactor/adr-141-windows-enterprise-sandbox-support.md) — Windows enterprise sandbox support
> - [ADR-145](../../docs/plans/architecture-refactor/adr-145-windows-sandbox-users-naming-decision.md) — OS 命名空间品牌一致（`Dasclaw*`）
>
> **关联 issue**：[#241](https://github.com/Linnanli/xClaw/issues/241) Windows OS sandbox epic、[#380](https://github.com/Linnanli/xClaw/issues/380) Batch 4 B4-3
>
> **本文档不涉及**：
> - 桌面客户端 UAC 弹窗 UX（=B4-2，独立 PR）
> - 真机矩阵 / AV 兼容测试（=B4-4，人工 VM lab）

---

## 1. 系统要求

| 项 | 最低 | 推荐 |
|---|---|---|
| OS | Windows 10 1809 (LTSC) | Windows 10 21H2+ / Windows 11 / Server 2019+ |
| 架构 | x86_64 | x86_64 |
| 权限 | 安装期需要本地管理员（UAC 提权） | 同 |
| 磁盘 | ≤ 50 MB 沙箱资源 | — |
| 杀软 | Defender 默认允许；360 / 瑞星需加入排除（见 §7） | — |

**运行期权限**：终端用户**无需**管理员。`dasclaw-windows-sandbox-setup.exe` 一次性以管理员身份创建本地组 `DasclawSandboxUsers` 后，普通用户即可启动沙箱。

---

## 2. 安装资源总览

setup.exe 创建以下系统级资源（命名空间见 [ADR-145 §6](../../docs/plans/architecture-refactor/adr-145-windows-sandbox-users-naming-decision.md)）：

| 类别 | 命名 | 用途 |
|---|---|---|
| 本地组 | `DasclawSandboxUsers` | 容纳 setup 创建的低权限 sandbox 用户 |
| 命名互斥 | `Local\DasclawSandboxReadAcl` | 跨进程 ACL 序列化写入 |
| 命名管道前缀 | `\\.\pipe\dasclaw-runner-*` | 提权 runner ↔ 主进程 IPC |
| 防火墙规则 | `DasclawSandboxOffline`、`DasclawSandboxOnline` | offline policy 出站拦截 |
| 防火墙友好名 | `Dasclaw Sandbox Offline - Block Non-Loopback Outbound`、`Dasclaw Sandbox Offline - Block Loopback UDP` | 同上（Windows 防火墙 UI 显示）|
| 私有桌面 | `DasclawSandboxDesktop-{:x}` | 隔离 sandbox 进程的 GUI |
| 资源目录 | `dasclaw-resources/`（**与 `dasclaw.exe` 同级目录**，由应用安装器放置） | helper binary / wildcard cert |
| Helper 二进制 | `dasclaw-command-runner.exe` | sandbox spawn 实际执行体 |

**每用户运行期资源**（首次启动沙箱时创建）：

| 路径 | 用途 |
|---|---|
| `%USERPROFILE%\dasclaw-home\.sandbox\` | 沙箱状态根（`CODEX_HOME` env 内部变量名见 §6）|
| `%USERPROFILE%\dasclaw-home\.sandbox\setup_marker.json` | setup 完成版本标记（SETUP_VERSION=5） |
| `%USERPROFILE%\dasclaw-home\.sandbox-bin\` | 解出的 helper binary |
| `%USERPROFILE%\dasclaw-home\.sandbox-secrets\sandbox_users.json` | DPAPI 加密的本地 sandbox 用户凭据 |

---

## 3. 安装方式

### 3.1 交互式（终端用户自助）

```powershell
.\dasclaw-windows-sandbox-setup.exe
```

弹出 UAC 同意后自动执行。setup 完成会写入 `setup_marker.json`；后续启动 `dasclaw.exe` 即可使用沙箱，无需再次提权。

### 3.2 Silent install（IT 推送 / 脚本化）

```powershell
.\dasclaw-windows-sandbox-setup.exe --silent
```

`--silent` 模式下：
- UAC 弹窗仍会出现（OS 强制，无法绕过），但**无需手工点击对话框中的内容**
- 失败信息写入 `%USERPROFILE%\dasclaw-home\.sandbox\setup-error.json`（per-user，结构见 §7.1）
- 退出码：`0` 成功 / `1` UAC 拒绝 / `2` 已是最新版（幂等成功）/ 其他 = 异常（查 setup-error.json）

要完全消除 UAC 对话框，必须通过下面任一管控渠道推送（让 OS 信任进程链）。

### 3.3 GPO（域环境）

1. 把 `dasclaw-windows-sandbox-setup.exe` 放到 SYSVOL 共享：
   `\\domain.local\SYSVOL\domain.local\Policies\{GUID}\dasclaw\`
2. 计算机配置 → 策略 → Windows 设置 → 脚本（启动/关机）→ 启动：
   ```
   程序：\\domain.local\SYSVOL\...\dasclaw-windows-sandbox-setup.exe
   参数：--silent
   ```
3. `gpupdate /force` 后下次重启生效，以 SYSTEM 身份运行（无 UAC 提示）

### 3.4 SCCM / Configuration Manager

应用程序模型 → 创建应用程序 → 手动指定：

| 字段 | 值 |
|---|---|
| 部署类型 | 脚本安装程序 |
| 安装程序 | `dasclaw-windows-sandbox-setup.exe --silent` |
| 卸载程序 | `dasclaw-windows-sandbox-setup.exe --uninstall --silent` |
| 检测方法 | PowerShell：`if (Get-LocalGroup -Name DasclawSandboxUsers -ErrorAction SilentlyContinue) { Write-Host 'Installed' }`（本地组存在 = 已安装；机器范围、可靠） |
| 安装行为 | 仅为系统安装 |
| 登录要求 | 用户是否登录均可 |

### 3.5 Intune（云管控）

打包成 `.intunewin`：

```powershell
IntuneWinAppUtil.exe -c <src-folder> -s dasclaw-windows-sandbox-setup.exe -o <out>
```

Intune 门户 → 应用 → Win32 应用：

- **安装命令**：`dasclaw-windows-sandbox-setup.exe --silent`
- **卸载命令**：`dasclaw-windows-sandbox-setup.exe --uninstall --silent`
- **检测规则**：自定义脚本（PowerShell）— `if (Get-LocalGroup -Name DasclawSandboxUsers -ErrorAction SilentlyContinue) { exit 0 } else { exit 1 }`
- **要求**：x64、Windows 10 1809+

---

## 4. UAC 行为说明

setup.exe 必须本地管理员身份才能：
- 创建本地组 `DasclawSandboxUsers`
- 注册 Windows 防火墙规则 `DasclawSandboxOffline` / `DasclawSandboxOnline`
- 安装 helper binary 到 `%ProgramData%\Dasclaw\dasclaw-resources\`

运行期 `dasclaw.exe` 启动沙箱**不需要**管理员；它通过 §2 表中的命名管道与已部署的提权 runner 通信。

---

## 5. 验证安装

```powershell
# 1. 检查本地组
Get-LocalGroup -Name DasclawSandboxUsers
Get-LocalGroupMember -Group DasclawSandboxUsers

# 2. 检查防火墙规则
Get-NetFirewallRule -DisplayGroup "Dasclaw Sandbox*" | Select-Object DisplayName, Enabled, Action

# 3. 检查 setup marker (任意已登录用户)
Test-Path "$env:USERPROFILE\dasclaw-home\.sandbox\setup_marker.json"

# 4. 端到端 echo 烟测
dasclaw.exe exec --policy workspace_write -- powershell -Command "Write-Host hello-from-sandbox"
```

预期输出：`hello-from-sandbox`。

---

## 6. 环境变量

> **注意**：以下 env 变量名 (`CODEX_HOME`) 沿用上游 [openai/codex](https://github.com/openai/codex) `6e838a19fa` 实现的字面量，per [ADR-145 §6 strict rules](../../docs/plans/architecture-refactor/adr-145-windows-sandbox-users-naming-decision.md) 不做 rename（变量名是上游 API 契约的一部分，与 OS 命名空间的品牌一致性区分对待）。终端用户可见的目录名仍是 `dasclaw-home`。

| 变量 | 默认 | 说明 |
|---|---|---|
| `CODEX_HOME` | `%USERPROFILE%\dasclaw-home` | 沙箱状态根 |
| `SANDBOX_ENABLED` | `true` | 总开关；`false` 时退化为 `direct` 模式 |
| `SANDBOX_EXECUTION_MODE` | `os_sandbox` | 见 [sandbox-activation.md](sandbox-activation.md) |
| `SANDBOX_POLICY` | `workspace_write` | `readonly` / `workspace_write` / `full_access` |
| `SANDBOX_ALLOW_FULL_ACCESS` | `false` | `full_access` 必须配合此变量为 `true`，否则降级 |

---

## 7. 故障排查

### 7.1 setup 失败：`setup-error.json`

setup 错误写入 per-user `CODEX_HOME` 下：

```
%USERPROFILE%\dasclaw-home\.sandbox\setup-error.json
```

IT 远程排查可读取（如已通过 SCCM 拉日志通道）；文件结构由 [`crates/dasclaw_sandbox_windows/src/setup_error.rs`](../../crates/dasclaw_sandbox_windows/src/setup_error.rs) `SetupErrorReport` 定义。

结构（已脱敏）：

```json
{
  "code": "GroupCreationFailed",
  "stage": "create_local_group",
  "win32_error": 1378,
  "message_redacted": "group [REDACTED] already exists with different SID"
}
```

常见 `code`：

| Code | 含义 | 处理 |
|---|---|---|
| `UacDenied` | 用户拒绝 UAC 同意 | 教育用户 / 用 §3.3-§3.5 推送 |
| `GroupCreationFailed` | 本地组名冲突 | 跑 §8 双名清理，再重试 |
| `FirewallRegistrationFailed` | 防火墙服务停止 | `Set-Service mpssvc -StartupType Automatic; Start-Service mpssvc` |
| `HelperMaterializationFailed` | helper 二进制写入失败（AV 拦截） | 加 §7.2 AV 排除项 |
| `DpapiSealFailed` | DPAPI 加密失败（无配置文件）| 确保 setup 以普通用户配置文件下的提权身份运行，**不要**用 `runas /user:System` |

### 7.2 杀软排除（Defender / 360 / 瑞星）

需要排除的路径 / 进程（路径取决于 `dasclaw.exe` 的实际安装位置，下例假设 `C:\Program Files\Dasclaw\`）：

```
<install-dir>\dasclaw-resources\
<install-dir>\dasclaw-resources\dasclaw-command-runner.exe
%USERPROFILE%\dasclaw-home\.sandbox-bin\
dasclaw.exe
dasclaw-windows-sandbox-setup.exe
dasclaw-command-runner.exe
```

Defender 示例：

```powershell
# 把 $installDir 改为 dasclaw.exe 实际安装根（如 "$env:ProgramFiles\Dasclaw"）
$installDir = "$env:ProgramFiles\Dasclaw"
Add-MpPreference -ExclusionPath $installDir
Add-MpPreference -ExclusionPath "$env:USERPROFILE\dasclaw-home"
Add-MpPreference -ExclusionProcess "dasclaw-command-runner.exe"
```

### 7.3 启动期 UAC 弹窗（不应出现）

如果终端用户在运行 `dasclaw.exe`（**非** setup.exe）时见到 UAC：
- 检查 §5 第 1/2 步是否都通过；若否，说明 setup 未在该机器执行过
- 检查 `%USERPROFILE%\dasclaw-home\.sandbox\setup_marker.json` 是否被 group policy 重定向到只读位置

---

## 8. 卸载流程

### 8.1 标准卸载

```powershell
.\dasclaw-windows-sandbox-setup.exe --uninstall --silent
```

会移除（机器范围）：
- 本地组 `DasclawSandboxUsers` 及其成员（含临时 sandbox 用户）
- 防火墙规则 `DasclawSandboxOffline`、`DasclawSandboxOnline` 及附属

**注意**：`dasclaw-resources/` 目录与 `dasclaw.exe` 同级，归 Dasclaw 应用主安装器管理（**不**由 sandbox setup.exe 创建）；如需移除请用应用主卸载流程。

**保留**（per-user 状态，不动）：
- `%USERPROFILE%\dasclaw-home\` — 用户工作产物（含 `.sandbox/setup_marker.json`、`.sandbox-bin/`、`.sandbox-secrets/sandbox_users.json`），需手动清理
- `setup-error.json` — 排查保留

### 8.2 双名清理（ADR-145 §8 OQ-145-3 弱防御）

> **背景**：x-claw 自 [PR #459](https://github.com/Linnanli/xClaw/pull/459) 起统一使用 `Dasclaw*` 命名空间。在此之前如果机器上曾以**预发布构建**或 fork 安装过任何 `Codex*` 命名的资源（生产环境理论上不存在，因为 #241 始终 `blocked`），用以下脚本一次性清理。**首发 B 路径机器无需运行此脚本**。

```powershell
#requires -RunAsAdministrator
# Dasclaw sandbox dual-name cleanup (per ADR-145 §8 OQ-145-3 weak-defence)
# Safe to re-run; idempotent.

$ErrorActionPreference = 'Continue'

# 1. Legacy + new local groups
foreach ($g in 'CodexSandboxUsers','DasclawSandboxUsers') {
    if (Get-LocalGroup -Name $g -ErrorAction SilentlyContinue) {
        Write-Host "Removing local group: $g"
        Remove-LocalGroup -Name $g
    }
}

# 2. Legacy + new firewall rules (match both display-group prefixes)
foreach ($prefix in 'Codex Sandbox','Dasclaw Sandbox') {
    Get-NetFirewallRule -DisplayName "$prefix*" -ErrorAction SilentlyContinue |
        ForEach-Object {
            Write-Host "Removing firewall rule: $($_.DisplayName)"
            $_ | Remove-NetFirewallRule
        }
}

# 3. ProgramData resource dirs (both names)
# 3. Per-user state (current user only; loop SIDs via Get-WmiObject if all-users sweep needed)
foreach ($d in "$env:USERPROFILE\codex-home","$env:USERPROFILE\dasclaw-home") {
    if (Test-Path $d) {
        Write-Host "Found per-user state: $d (NOT removed; delete manually if desired)"
    }
}

Write-Host "Cleanup complete."
```

执行后 §5 验证步骤应全部返回空 / `False`。

### 8.3 完全清理（含 per-user 状态）

每位终端用户在自己会话下：

```powershell
Remove-Item -LiteralPath "$env:USERPROFILE\dasclaw-home" -Recurse -Force
```

该目录包含 `.sandbox/`、`.sandbox-bin/`、`.sandbox-secrets/` 三个子目录及 setup error report。

---

## 9. 附录

### 9.1 setup 资源 → ADR-145 映射

setup.exe 创建的每个 OS 资源命名都可追溯到 [`scripts/codex_to_dasclaw_rename_map.json`](../../scripts/codex_to_dasclaw_rename_map.json) 中的一条 `literal_replacements`。该映射表会被未来的 drift guard（B4-1）用于校验 `crates/dasclaw_sandbox_windows/` 与 `vendor/codex-windows-sandbox/upstream/` 的字符级一致性。

### 9.2 不变量（per ADR-145 §6 strict rules）

以下字面量**保留 codex 上游原文**，运维不必关心，但 IT 排查时若在日志见到属正常：

- 源码注释 `// Derived from openai/codex commit 6e838a19fa`（per-file 归属，Apache-2.0 attribution）
- env 变量名 `CODEX_HOME`（上游 API 契约）
- 测试 fixture 变量名 `codex_home`（仅源码内部，不暴露给用户）
- panic 信息 `"create codex home"`（绑定变量名，仅测试断言）

### 9.3 已知 Gap：RestrictedToken / Unelevated fallback 未端到端启用

**追踪 issue**：[#462 — Wire `WindowsSandboxLevel::RestrictedToken` end-to-end](https://github.com/Linnanli/xClaw/issues/462)

[`WindowsSandboxLevel`](../../crates/dasclaw_protocol/src/config_types.rs) 枚举定义了三档：`Disabled` / `RestrictedToken` / `Elevated`。底层 `dasclaw_sandbox_windows::run_windows_sandbox_legacy_preflight`（codex `WindowsSandboxSetupMode::Unelevated` 对应实现，verbatim port）也已 export。但当前 `dasclaw_sandboxing::manager` 调度仅区分 `Disabled` vs 「非 Disabled」，**未按 `RestrictedToken` 分流到 legacy preflight 路径**。

**对部署的影响**：

- 用户拿不到 admin 凭据 / 组策略禁用 UAC 提权 / 没有 GPO/SCCM/Intune 基础设施时
- 当前行为：setup.exe UAC 被拒后**沙箱启用失败**，不会按设计自动降级到 RestrictedToken
- 设计预期：自动降级到无 admin、纯 ACL+受限令牌+env 桩的弱保护模式

**临时绕过**：

- 政企用户：走本文 §3.3–3.5 的 GPO/SCCM/Intune **静默预部署**（SYSTEM 身份跑 setup.exe，开发者机器零 UAC）
- 个人用户：必须有 admin（或可暂时 sudo 一次）

详见 issue #462 的 Acceptance 列表。

---

**变更日志**

| 日期 | PR | 说明 |
|---|---|---|
| 2025-11-15 | (本 PR) | 初版；ADR-145 落地后首份 IT 部署文档；§9.3 记录 RestrictedToken gap → issue #462 跟进 |
