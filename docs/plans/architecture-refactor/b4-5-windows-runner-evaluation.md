# B4-5：Windows sandbox e2e CI runner 评估（hosted vs self-hosted）

> **Status**: Draft（agent 起草，待人签）
> **Date**: 2025-12-13
> **Owner**: nally（产品 + 工程）
> **Issue**: [#466](https://github.com/Linnanli/xClaw/issues/466)
> **Epic**: [#380](https://github.com/Linnanli/xClaw/issues/380) Batch 4 / B4-5
> **Refs**: [#241](https://github.com/Linnanli/xClaw/issues/241) Phase 5、[ADR-145](adr-145-windows-sandbox-users-naming-decision.md)、[B4-3 政企部署指南](../../../desktop-client/docs/windows-sandbox-setup-guide.md)

---

## 1. 问题

`crates/dasclaw_sandbox_windows` 是从 codex verbatim 移植的 Windows OS sandbox。
真正的端到端验证需要在一个**真实 Windows 主机**上跑：

1. 安装 `dasclaw-windows-sandbox-setup.exe`（要求 admin + UAC）
2. setup 流程会调用 `net user` 创建 `DasclawSandbox{Offline,Online}` 本地用户，`net localgroup`
   创建 `DasclawSandboxUsers` 本地组，写 `netsh advfirewall` 4 条 firewall 规则，
   写 DPAPI 密钥，落 manifest 到 `%ProgramData%\dasclaw\`
3. 跑 sandbox 化的 echo / PowerShell（Restricted Token + Job Object + 隔离桌面 +
   ConPTY）
4. uninstall 清掉 §2 全部 OS 资源

问题：**这段 e2e 能否跑在 GitHub Actions hosted `windows-latest` runner 上？**
**不能则需要什么形态的自托管 runner？**

## 2. `windows-latest` runner 能力清单（hosted）

> Source：[GitHub-hosted runners docs](https://docs.github.com/en/actions/using-github-hosted-runners/about-github-hosted-runners/about-github-hosted-runners) +
> [`actions/runner-images` Windows2022-Readme.md](https://github.com/actions/runner-images/blob/main/images/windows/Windows2022-Readme.md)（撰写时为 ws2022 镜像）。
> 自动化无法访问外网时，以下结论需由人 hands-on 验证。

| 能力 | hosted `windows-latest` | 备注 |
|---|---|---|
| Admin 权限 | ✅ runner 进程以 `Administrator` 用户运行 | 直接可跑 `setup.exe` 而**无须 UAC 弹窗**（已是 elevated session） |
| `net user` 创建本地账号 | ✅ 支持 | 但 hosted 镜像每次 job 都是全新 VM，无需 uninstall 也会被 GitHub 回收 |
| `net localgroup` 创建本地组 | ✅ 支持 | 同上 |
| `netsh advfirewall` 增删规则 | ✅ 支持 | 同上 |
| Windows Defender | ⚠️ 启用、实时保护开 | 创建本地用户、改 firewall 是 admin 合法操作，**通常不会拦**，但 Defender 可能对 setup.exe 做云查杀延迟（首次几十秒） |
| DPAPI | ✅ 支持，runner user profile 完整 | DPAPI 密钥绑定到 runner user，job 结束随 VM 销毁 |
| ConPTY | ✅ Windows Server 2022 有 ConPTY | 与客户端 SKU 同 API 表层；不过 ConPTY 在 ws2022 vs Win10/Win11 桌面 SKU 行为有细微差异（焦点 / Win+Tab 等不适用于无 GUI 场景） |
| Alternate Desktop（Win32 `CreateDesktop`） | ⚠️ Server Core 上行为受限，**Desktop Experience SKU 应可用**；hosted ws2022 是 Desktop Experience | 沙箱内的 `dasclaw-command-runner.exe` 隔离桌面创建需要此 API |
| Win+R 风险弹窗 / Toast | ❌ 无图形会话 | 沙箱本身不需要 GUI，但 `assert_no_user_input` 等 smoke 不可能跑 |
| 跨 job 持久化 | ❌ 每个 job 一台全新 VM | 测 `setup → uninstall → setup again` 需在同一 job 内串行 |
| Job 时长上限 | 6 小时 / job | 单个 e2e（setup+echo+uninstall）≤ 10 分钟，无压力 |
| 并发 | hosted 配额内 ~20 并发 | 矩阵 (setup×AV×SKU) 容量充足 |
| 网络访问 | ✅ 可访 GitHub / crates.io / 系统包源 | sandbox 测试本身不需要外网 |
| ARM64 | ❌ hosted 仅 amd64 | 与 Windows sandbox 目标客户重叠（amd64 LTSC），暂不关心 |

**SKU 缺口**：hosted 只有 **Windows Server 2022 Desktop Experience**（amd64）。我们的政企客户实际跑在：
- Windows 10 LTSC 2019 (1809)
- Windows 10 LTSC 2021 (21H2)
- Windows 11 23H2 / 24H2
- Windows Server 2019 / 2022 Desktop Experience

→ hosted runner 能覆盖 **API surface 与 ConPTY 下限**，但**不能覆盖 Win10 1809 ConPTY 早期 bug、桌面 SKU UAC quirks、客户端 SKU 特有的 Defender for Endpoint 策略**。

## 3. 决策矩阵

| 验证目标 | hosted runner 够用？ | 必须 self-hosted？ |
|---|---|---|
| `cargo check / clippy / test (单测)` Windows target | ✅ 现已跑（`windows-ci.yml`） | 否 |
| `dasclaw-sandbox-setup.exe` 创建 user/group/firewall/DPAPI 全链路 | ✅ admin 已有；可跑 | 否（hosted 即可） |
| 沙箱化 echo / PowerShell smoke | ✅ ConPTY OK | 否（hosted 即可） |
| Uninstall 清理回归 | ✅ 同 job 串行 | 否（hosted 即可） |
| Win10 1809 / 21H2 / Win11 桌面 SKU 覆盖 | ❌ hosted 无桌面 SKU | **是** |
| Defender / 360 / 瑞星 三家 AV 误报矩阵 | ❌ hosted 只 Defender 默认配置 | **是**（B4-4） |
| 真机硬件特性（TPM / Secure Boot / Credential Guard） | ❌ hosted VM 简化 | **是**（B4-4） |

**结论**：
- **B4-5 短期产出**：先在 hosted `windows-latest` 上拉起一个 `sandbox-e2e` workflow，跑
  setup → echo → PowerShell smoke → uninstall。这个能跑通就把 #241 Phase 5 的 CI gate
  **机器可验证那部分**先封顶。
- **B4-5 长期产出**：再投资 1 台 self-hosted runner（Win10 LTSC 2021 VM，Defender 默认配置）做桌面 SKU 兜底回归；AV 矩阵（360 / 瑞星）走 B4-4 人工 / 季度回归而非 CI。

## 4. 推荐架构

```
┌──────────────────────────────────────────────────────────────────┐
│  GitHub Actions hosted (windows-latest = ws2022 Desktop Exp.)    │
│  ┌────────────────────────────────────────────────────────────┐  │
│  │ windows-sandbox-e2e job (push / weekly schedule)           │  │
│  │   1. cargo build -p dasclaw_sandbox_windows --release --bin│  │
│  │      dasclaw-windows-sandbox-setup --bin                   │  │
│  │      dasclaw-command-runner                                │  │
│  │   2. .\dasclaw-windows-sandbox-setup.exe --silent --yes    │  │
│  │   3. assert sandbox_setup_is_complete()                    │  │
│  │   4. 跑沙箱 echo / powershell smoke                        │  │
│  │   5. .\dasclaw-windows-sandbox-setup.exe --uninstall       │  │
│  │   6. assert net localgroup DasclawSandboxUsers == not exist │  │
│  └────────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────────┐
│  Self-hosted (Phase 5b，待 B4-5 hosted 通过后启动；本 issue 不实施)│
│  - 1 台 Win10 LTSC 2021 VM（amd64，Hyper-V）                       │
│  - GitHub Actions self-hosted runner，label: `windows-ltsc-2021`  │
│  - 隔离子网，仅出向 github.com / crates.io                          │
│  - 定时回归（weekly）；PR-触发仅在 `windows-ltsc-2021` label 时跑    │
│  - 设定 ephemeral runner，每 job 后从 snapshot 回滚到干净状态        │
└──────────────────────────────────────────────────────────────────┘
```

## 5. 风险与缓解

| 风险 | 缓解 |
|---|---|
| hosted runner Defender 首次扫描 setup.exe 慢 / 误报 | setup.exe 经 Apache-2.0 attribution；CI step 前先 `Add-MpPreference -ExclusionPath`（仅 hosted VM；真机不做） |
| hosted runner 跨 job 状态泄漏 | hosted 每 job 全新 VM，无问题；self-hosted 用 ephemeral 模式 |
| self-hosted runner 长期托管成本（人 / 网络 / 凭据） | 等 hosted runner gate 稳定后再投；最小 1 节点即可 |
| GitHub Actions windows-latest 升级到 ws2025 时 API 行为漂移 | `windows-ci.yml` 已 pin `runs-on: windows-latest`；漂移时 verbatim drift guard（B4-1，已合并）会先抓 crate 改动，CI flake 由人工 triage |
| sandbox setup 期间触发 Defender 云查 → job 偶发超时 | 单 job 加 `timeout-minutes: 15`；retry 1 次；连续 3 次失败再告警 |

## 6. 下一步 PR 拆分

| ID | 范围 | 依赖 |
|---|---|---|
| **B4-5a**（下个 PR） | 新增 `.github/workflows/windows-sandbox-e2e.yml`：在 hosted `windows-latest` 上跑 setup → echo smoke → uninstall。先只挂 `workflow_dispatch` + `schedule(weekly)`，不阻断 PR | 本评估文档（B4-5）合并 |
| **B4-5b** | hosted 通过 1 周后，把 `sandbox-e2e` 接进 `windows-ci.yml`，对 sandbox crate paths 触发 | B4-5a |
| **B4-5c** | self-hosted Win10 LTSC 2021 runner 部署 + 接 CI | B4-5b 稳定；人 + IT 协作 |
| **B4-4** | 真机矩阵 smoke 报告（4 SKU × 3 AV，人工） | 独立于 CI，可与 B4-5b/c 并行 |
| **B4-6** | 关闭 #241 + 解除 #128 Windows blocker | B4-2 / B4-3 / B4-4 / B4-5b 全部完成 |

## 7. 决策

- **接受**：先走 hosted `windows-latest`，落地 B4-5a workflow；同时立 self-hosted runner 部署
  ticket（B4-5c）但不立即实施，等 hosted gate 稳定后再投资。
- **拒绝**：不一开始就拉 self-hosted runner（成本高，且 hosted 已能覆盖 95% 自动化场景）。
- **拒绝**：不把 AV 矩阵纳入 CI（属 B4-4 人工 / 季度）。

---

## 附录 A — hosted runner 能力声明的引用线

撰写本评估时 agent 未联网。以下结论需由签字者 hands-on 复核：

- runner 用户名通常为 `runneradmin` 或 `RUNNER`（GitHub-hosted），属本地 Administrators 组。  
  验证命令：`whoami /groups` 看是否含 `Administrators`。
- `net user dasclawsandboxoffline_test /add` 是否需要交互式 confirmation。
- `netsh advfirewall firewall add rule name="x" dir=out action=block` 是否被 hosted runner 默认策略拒绝。
- `CreateDesktop` API 是否在 hosted ws2022（headless）能创建非默认桌面。

若任何一项失败，回退到自托管 runner 即为强制路径（B4-5c 提前到 B4-5a）。
