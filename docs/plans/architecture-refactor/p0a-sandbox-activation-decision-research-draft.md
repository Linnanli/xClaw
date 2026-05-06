# P0-A 沙箱激活决策 — 研究草稿（人类审阅前）

> **Status**: 🟡 **Research draft, NOT an Accepted ADR** — 由 agent 按 [`AGENTS.md`](../../../AGENTS.md) §"分析工具使用规范" 三层验证后产出，**未经人类签字**。本文件**不是** [#127](https://github.com/Linnanli/xClaw/issues/127) 要求的 `adr-XXX-p0a-sandbox-activation-decision.md` ADR 文件，仅是给 ADR 作者的输入材料。

---

## ⭐ 决策最终结果摘要（本会话最终，覆盖正文 §2-§8）

> **重要**：正文 §2-§8 是**研究阶段**的初版建议（基于已 SUPERSEDED 的 D0=B 假设）。本会话经多轮 per-decision MCP gate 评审后，**最终决策矩阵以本节 + §附录 A 为准**。ADR 作者请按此摘要 + 附录 A 写正式 ADR，正文 §2-§8 仅作研究历史保留。

| # | 决策点 | 最终选择 | 关键约束 |
|---|---|---|---|
| **D0** | ExecutionMode 字段命名 | **C — `ExecutionMode { Direct, OsSandbox, Docker }` 枚举**（替代 boolean toggle，Direct 仅 dev 用，不暴露给终端用户） | 政企客户端只暴露 OsSandbox + Docker 两选项 |
| **D1** | enterprise 部署默认行为 | **B — enterprise 强制开启，无降级路径**（fail-CLOSED） | 不允许 fail-OPEN 后门 |
| **D2** | Rollout 策略 | **E + 友好错误子系统** — 直接全量激活，配套友好错误信息子系统给终端用户引导 | 与 D4a=D 联动 |
| **D3** | Windows 平台实施路径 | **D3-3 — Fork upstream `codex-windows-sandbox` 进 monorepo（方案 X）**（详见 §附录 A） | 用户隔离 + JO + RT + Firewall + Desktop 隔离 + DPAPI + ConPTY；Apache-2.0；最低 Win10 1809 |
| **D4a** | 沙箱**初始化失败**时桌面行为 | **D — 启动 desktop + setup 引导**（不阻断对话能力，但 ShellTool 不注册） | Windows 首次部署 100% 命中此路径，是默认主流程 |
| **D4b** | 沙箱**运行时拒绝命令**时行为 | **A — 友好错误说明被拦截**（不自动 escalate，fail-CLOSED） | 与 D2 友好错误子系统共用 |
| **D5** | SandboxPolicy 在沙箱激活时默认值 | **E — `WorkspaceWrite` 默认 + 政策文件可覆盖** | 默认与 codex 上游一致；政企 IT 可通过 GPO 下发 ReadOnly 收紧；`DangerFullAccess` 仍需 `SANDBOX_ALLOW_FULL_ACCESS=true` 双重 opt-in |
| **D6** | proxy enable 与 sandbox toggle 关系 | **A — proxy 与 sandbox 共用一个开关，不可独立 toggle**（**简化版，不带 `always_on` 后门**） | `start_network_proxy` 与 `OsExecutor::new` 在同一激活点（[#128](https://github.com/Linnanli/xClaw/issues/128)）调用 |
| **D7** | 迁移窗口 | **A — 不迁移 / 直接切换**（当前 0 生产调用方，无旧行为可迁移） | changelog / release notes 明确写新行为；Win UAC setup 走 D4a=D 软引导 |

### Framework 实施策略（A→C 渐进，与决策矩阵正交）

- **W3（[#128](https://github.com/Linnanli/xClaw/issues/128)）= 档位 A**：~200 LOC，env-driven + Builder API fallback，最小激活范围
- **W4（待新 ADR）= 档位 C**：~1500 LOC，纯 Builder API，lib 完全脱离 Settings；W3 完成后开新 issue
- **W3-W4 之间**：Windows 平台启动方案 X（fork codex-windows-sandbox），见 [`p0a-sandbox-fork-codex-windows-sandbox-issue-draft.md`](./p0a-sandbox-fork-codex-windows-sandbox-issue-draft.md)

### 政企客群定位（决策约束 — 不可改变）

- **Case A（90% 场景）**：终端用户在本地 Windows 上用 desktop-client 操作本地 Word / Excel / PowerShell / 工具链
- **私有云沙箱不适合**：会破坏"本地文件无缝访问"体验
- **政企招标硬约束**：必须有技术沙箱（不只是权限审批），与 Cursor / Trae 走开发者付费市场策略不同
- **fail-CLOSED 不可妥协**：违反 D1=B 强制原则的方案（如 fail-OPEN 后门）一律否决

### 已 SUPERSEDED 的旧矩阵

> 之前 commit 历史里 [`/memories/repo/x-claw-notes.md` line 118](2026-05-05) 记录的"用户已批准 8/8 建议（D0=B / D1=C / D2=E+C / D3=D Docker / D4=B / D5=WW / D6=A / D7=A）"已 SUPERSEDED。本会话用户重新评审后得到上表新矩阵，主要差异：
> - D0：B（独立命名空间）→ **C（ExecutionMode 枚举）**
> - D1：C（enterprise 强制 + 个人可降级）→ **B（无降级，更严）**
> - D2：E+C → **E + 友好错误子系统**
> - D3：D（Windows 走 Docker）→ **D3-3（fork codex-windows-sandbox）**（基于 §附录 A 实地考察修正）
> - D4：B → **D4a=D + D4b=A**（拆分 init / runtime 语义）
> - D6：A 含 always_on 后门 → **A 简化版无后门**
> - D5、D7：保持原选

---

> **Purpose**: 为 #127（adr-redline）梳理 7 个待决策点，并对每一点给出：
> - 现状证据（来自 [`p0a-sandbox-activation-inventory.md`](./p0a-sandbox-activation-inventory.md)）
> - 候选选项（含对照 codex 上游做法）
> - 各选项的 trade-off
> - **agent 的初步建议（非约束、需人类拍板）**
>
> **Scope guardrails**:
> - ❌ 本文件不修改任何 Rust 代码
> - ❌ 本文件不创建 `adr-XXX-...md`（因 ADR 必须人类 Accepted 签字）
> - ❌ 本文件不影响 [#128](https://github.com/Linnanli/xClaw/issues/128) 实现 issue 的 acceptance 边界
> - ✅ 仅产出 markdown 研究材料供 [#127](https://github.com/Linnanli/xClaw/issues/127) 决策人参考
>
> **Source / refs**:
> - 决策 issue（红线）：[#127](https://github.com/Linnanli/xClaw/issues/127)
> - 父 issue：[#28](https://github.com/Linnanli/xClaw/issues/28)
> - inventory（已 Closed，证据基础）：[#126](https://github.com/Linnanli/xClaw/issues/126) → [`p0a-sandbox-activation-inventory.md`](./p0a-sandbox-activation-inventory.md)
> - 实现 issue（被本决策阻塞）：[#128](https://github.com/Linnanli/xClaw/issues/128)
> - Windows 计划交叉引用：[#91](https://github.com/Linnanli/xClaw/issues/91)
> - Net-proxy port ADR：[`43-net-proxy-port-adr.md`](./43-net-proxy-port-adr.md)
> - Net-proxy port 完成报告：[`44-net-proxy-port-completion.md`](./44-net-proxy-port-completion.md)
> - Resource limits ADR：[`45-resource-limits-adr.md`](./45-resource-limits-adr.md)
> - Docker vs OS sandbox 对照：[`41-docker-vs-os-sandbox-capability-comparison.md`](./41-docker-vs-os-sandbox-capability-comparison.md)

---

## 0. 关键背景（决策前必须读）

### 0.1 当前已有「两套独立」沙箱配置类型 — 这是本次决策的最大语义陷阱

`desktop-client/ironclaw` 同时存在两个名字非常相似的 `SandboxConfig` 类型：

| 类型 | 文件 | 用途 | 当前是否已激活 |
|---|---|---|---|
| `config::SandboxModeConfig` | [`desktop-client/ironclaw/src/config/sandbox.rs:6`](../../../desktop-client/ironclaw/src/config/sandbox.rs) | **Docker** sandbox 模式（容器执行）的配置：image、reaper、orphan、cpu_shares、memory_limit_mb、`extra_allowed_domains` 等 | ✅ 已生产使用，`enabled` 默认 `true`，env `SANDBOX_ENABLED` 可改 |
| `sandbox::SandboxConfig` | [`desktop-client/ironclaw/src/sandbox/config.rs:7`](../../../desktop-client/ironclaw/src/sandbox/config.rs) | **OS** sandbox（macOS Seatbelt / Linux Landlock+seccomp）+ HTTP forward proxy 的配置：`policy`、`network_allowlist`、`proxy_port` 等 | ❌ 0 生产调用方，仅测试 fixture 使用 |

**冲突字段**：两者都有 `enabled`、`policy`、`allow_full_access`、`memory_limit_mb`、`cpu_shares`、`timeout`，语义在两个层次上：

- Docker sandbox = 容器级隔离（worker 进程在 Docker 里跑）
- OS sandbox = 进程级隔离（worker 进程在宿主机跑，但被 seatbelt/landlock 限制）

两套**没有合并语义**。当前 [`app.rs:444`](../../../desktop-client/ironclaw/src/app.rs) 唯一引用是 `if self.config.builder.enabled && (self.config.agent.allow_local_tools || !self.config.sandbox.enabled)`，这里的 `config.sandbox` 指 **`SandboxModeConfig`**（Docker），与 OS 沙箱无关。

> **决策点 0**（隐式）：**新增的「OS 沙箱激活开关」是否复用 `SandboxModeConfig.enabled` 字段，还是新增独立字段（如 `os_sandbox.enabled`）？**
>
> 这一点 #127 列表里没有显式列出，但 [#127 决策清单第 1 条](https://github.com/Linnanli/xClaw/issues/127) "Default value of `config.sandbox.enabled`" 措辞就直接用了 `config.sandbox.enabled`，存在歧义。**强烈建议人类先回答这一点**，否则后续 6 条决策都受其影响。

### 0.2 当前激活差距：3 个关键 0 调用方

来自 [inventory §3](./p0a-sandbox-activation-inventory.md)：

- `OsExecutor::new(...)` — 4 处匹配，0 个生产调用
- `start_network_proxy(...)` — 6 处匹配，0 个生产调用
- `ShellTool::new()` 唯一生产注册点（[`tools/registry.rs:480`](../../../desktop-client/ironclaw/src/tools/registry.rs)）**不调用** `with_sandbox` / `with_sandbox_policy` / `with_extra_env`

→ 整套 OS 沙箱 + HTTP proxy 链路**完全休眠**。激活 = 把 3 个零调用方变成有调用方。

### 0.3 `ShellTool` 当前默认 = `SandboxPolicy::ReadOnly`

`ShellTool::new()` 默认 `sandbox: None, sandbox_policy: SandboxPolicy::ReadOnly`。当 `sandbox: None` 时走 `execute_direct()`，直接 `sh -c` 无任何隔离。`sandbox_policy` 字段当前**只在 `sandbox: Some(executor)` 路径下被消费**。

### 0.4 codex 上游对照（仅作参考，不约束）

来自 [`codex-cli-main/codex-rs/core/src/session/tests.rs`](../../../codex-cli-main/codex-rs/core/src/session/tests.rs)：

- codex 测试场景**默认使用 `SandboxPolicy::new_workspace_write_policy()`**，即 `WorkspaceWrite`，而不是 `ReadOnly`。
- codex `Constrained::allow_any(SandboxPolicy::DangerFullAccess)` 是显式 opt-in 的逃生口，不是默认。
- codex `Session::start_managed_network_proxy` 是无条件在生产 boot 路径上启动的（参考 [`44-net-proxy-port-completion.md`](./44-net-proxy-port-completion.md)），并非 flag 控制。

---

## 1. 七个决策点（#127 列表逐条）

下表是 [#127 Decisions to make](https://github.com/Linnanli/xClaw/issues/127#decisions-to-make) 列出的 7 项，按 ADR 决策依赖图重排（前置依赖在前）：

| # | #127 原条目 | 决策依赖于 |
|---|---|---|
| **D0** | (隐式) 字段命名空间：复用 `SandboxModeConfig.enabled` 还是新增 `os_sandbox.enabled` | 无 |
| **D1** | Default value of `config.sandbox.enabled` (`false` flag-off vs `true` forced) | D0 |
| **D2** | Rollout strategy（默认开 / 默认关 / opt-in / 首次启动 prompt） | D1 |
| **D3** | Windows behavior（实现 / 声明不支持 / 走替代执行模型） | 独立，但与 #91 联动 |
| **D4** | Fail-closed 语义（沙箱不可用时的错误信息 + 是否完全禁用 shell） | D1 + D3 |
| **D5** | Default `SandboxPolicy`（`ReadOnly` / `WorkspaceWrite` / 其他） | D1 |
| **D6** | Proxy 启用条件（与沙箱共开关 vs 独立 toggle） | D0 + D1 |
| **D7** | 迁移窗口（现有用户 config 是否需要迁移脚本） | D0 |

后文按 D0 → D7 顺序展开。

---

## 2. D0 — 字段命名空间（**前置**，强烈建议优先决策）

### 2.1 现状

- `Settings.sandbox: SandboxModeConfig`（Docker 模式配置，已生产使用）
- `Settings.sandbox.enabled: bool` 默认 `true`（[`config/mod.rs:177-179`](../../../desktop-client/ironclaw/src/config/mod.rs)）
- `crate::sandbox::SandboxConfig`（OS 模式 + proxy 配置，0 生产引用）

### 2.2 候选选项

| 选项 | 描述 | 优点 | 缺点 |
|---|---|---|---|
| **A. 直接复用 `SandboxModeConfig.enabled`** | 让一个 toggle 同时控制 Docker mode 和 OS sandbox。`enabled = true` 时同时启用容器隔离 + OS 沙箱 + proxy | 用户配置最简单 | 语义糅合：原 `SandboxModeConfig` 描述 Docker 容器，而 OS 沙箱是宿主机进程级隔离，两者**互斥**（容器内不需要再套 seatbelt）。强行复用导致字段语义不可解释 |
| **B. 新增独立 `Settings.os_sandbox: OsSandboxConfig`** | 完全独立的命名空间，与 `Settings.sandbox`（Docker）平行 | 语义清晰；两层独立可独立开关；为未来废弃 Docker 路径留下退路 | 用户需要理解两个 toggle |
| **C. 引入第三层 enum 字段 `Settings.execution_mode: ExecutionMode { Direct, OsSandbox, Docker }`** | 三选一：直接执行 / OS 沙箱 / Docker 沙箱 | 语义最干净；天然 mutually exclusive；逐项扩展不冲突 | 重构成本最高；需要把 `SandboxModeConfig.enabled` 迁移成 `ExecutionMode::Docker` 分支 |

### 2.3 codex 对照

codex 没有 Docker mode（不存在 `SandboxModeConfig`），所以这个决策在 codex 不适用。

### 2.4 Trade-off 摘要 + 建议

- **选项 A** 把语义债务永久化，违反 [`AGENTS.md`](../../../AGENTS.md) "禁止补丁式代码"
- **选项 C** 是「正确但代价大」的架构治理路径，但当前 W3 wave 优先级是**激活而非重构**
- **选项 B** 是「最小改动 + 语义清晰」的实用解

> **🟢 agent 建议**：**选项 B（独立命名空间 `os_sandbox`）**。
>
> 理由：(1) 不破坏现有 `SandboxModeConfig` 用户配置；(2) 给 #128 实现 issue 的最小改动是新增字段而非重命名；(3) 给 D7「是否需要迁移脚本」让出空间（独立字段不需要迁移现有 `SANDBOX_ENABLED` 行为）；(4) 选项 C 可在 W4+ 单独开 ADR 做（属于架构治理改动，不应阻塞 W3 红线）。

---

## 3. D1 — Default value of `os_sandbox.enabled`（假设 D0=B）

### 3.1 选项

| 选项 | 描述 | Blast radius |
|---|---|---|
| **A. 默认 `false`，opt-in 启用** | 现有用户行为不变；新用户必须显式开 | 低（不影响存量） |
| **B. 默认 `true`，flag-off 退出** | 默认启用；`curl https://api.example.com` 等命令立即失败需经 proxy | 高（直接打破现有行为） |
| **C. 默认 `true` 但仅在 enterprise mode 启用** | 取决于 `is_enterprise_mode()` 判断（admin policy 推送场景） | 中（仅企业用户受影响） |

### 3.2 #28 背景

[#28 "Why deferred (user-confirmed option A)"](https://github.com/Linnanli/xClaw/issues/28) 已显式记录：

> Activating the sandbox is a **user-visible behavior change**: ShellTool currently bare-spawns child processes with full host access. ... This requires:
> - A graceful flag-off-by-default rollout, or
> - Explicit user-facing release notes + opt-in config

#28 author 已倾向 flag-off rollout，但未正式 ADR 化。

### 3.3 Trade-off

- 选项 A 安全但弱化沙箱价值（多数用户永远不开）
- 选项 B 安全姿态最强但首次发版即破坏 50%+ 现有 shell 命令体验
- 选项 C 与 #95（enterprise tenant fail-closed 契约）天然契合

> **🟢 agent 建议**：**选项 C（默认 `false`，但 enterprise mode 强制 `true` 且不可关）**。
>
> 理由：(1) 满足 P0-A 的 "enterprise mode fail-closed" 契约（#28 标题原意）；(2) 不破坏个人开发者用户的现有行为（兜底选项 A 的体验）；(3) 与 #95 的 fail-closed 契约自然衔接；(4) 把"默认行为"和"安全合规"分开决策，避免把安全负担压在所有用户头上。
>
> **若选 C，建议下游约束**：`SandboxConfig.allow_full_access = true` 在 enterprise mode 下必须被强制忽略（已有 double-opt-in 机制，参考 [`config/sandbox.rs:78-87`](../../../desktop-client/ironclaw/src/sandbox/config.rs)）。

---

## 4. D2 — Rollout strategy

### 4.1 选项

| 选项 | 描述 |
|---|---|
| **A. silent default-on**（不通知） | 沙箱默认启用，无 release notes，用户感知差 |
| **B. silent default-off** | 默认关闭，用户自己看 docs |
| **C. opt-in via release notes** | 默认关闭，发版时发 release notes 引导 enterprise 用户启用 |
| **D. explicit user prompt on first run** | 首次启动弹窗询问是否启用 |
| **E. enterprise-mode-only forced** | enterprise mode 强制启用（与 D1 选项 C 联动） |

### 4.2 现实约束

- desktop-client 是 Tauri 桌面端，**有 UI 交互能力**，可以做 D 选项的弹窗
- 但企业模式的策略是 **admin 推送**（参考 [#95](https://github.com/Linnanli/xClaw/issues/95)），用户不应有 opt-out 能力 → 排除 D 选项的"询问"设计

### 4.3 Trade-off

> **🟢 agent 建议**：**E + C 组合**。
>
> 具体：
> - **enterprise mode**（admin policy 标记）：强制启用、无 UI、无 release notes 用户引导（admin 自己决定何时切换）
> - **个人开发者模式**：默认关闭 + release notes 引导 + docs 入口
>
> 排除 silent on/off：silent on 破坏体验，silent off 让安全 feature 永远没人开。

---

## 5. D3 — Windows behavior

### 5.1 现状（来自 [inventory §9](./p0a-sandbox-activation-inventory.md)）

- `crates/dasclaw_sandbox` 只有 `pub mod macos` + `pub mod linux`，**没有 `pub mod windows`**
- `SandboxType::WindowsRestrictedToken` enum 变体存在但 `NoopSandbox::execute` 直接返回 `SandboxError::NotImplemented { stage: 4, detail: "windows restricted token backend lands in W2.4" }`
- Windows 平台目前 **silently 走 `OsExecutor` direct 路径**（无沙箱）

### 5.2 选项

| 选项 | 描述 | 与 #91 关系 |
|---|---|---|
| **A. 实现 Windows backend**（Job Object + Restricted Token） | W2.4 完整 Windows 沙箱 | 等于把 #91 一起做 |
| **B. 声明 Windows 不支持 enterprise mode**（安装时阻断） | enterprise 用户必须用 macOS/Linux | 与 #91 划清边界 |
| **C. Windows = 不沙箱但仍走 proxy + 显式警告** | 沙箱 fail-open，但网络出口仍受 proxy 约束 | #91 处理"warning" 文案 |
| **D. Windows 走替代执行模型**（如：必须走 Docker mode = `SandboxModeConfig`） | enterprise + Windows = 强制容器模式 | 与 D0 选项 A 矛盾 |

### 5.3 Trade-off

- 选项 A：实现成本最高（W2.4 已声明 deferred，#91 已立 P1）
- 选项 B：最严格但断送 Windows enterprise 客户
- 选项 C：fail-open 违反 P0-A "fail-closed" 契约 → **直接淘汰**
- 选项 D：把 Windows enterprise 推到 Docker 容器，复用现有路径，但要求 Windows 用户装 Docker Desktop

> **🟢 agent 建议**：**B + D 二选一，倾向 D**。
>
> **B（声明不支持）**：最干净，但 PR 要求 enterprise installer 在 Windows 上拒绝执行 + 提供清晰的"请改用 macOS/Linux"指引。
>
> **D（强制走 Docker）**：复用 [`SandboxModeConfig`](../../../desktop-client/ironclaw/src/config/sandbox.rs) 的成熟 Docker 路径。Windows enterprise 用户必须装 Docker Desktop，但**不需要等 #91 完成**就能 ship enterprise mode。
>
> **拒绝 C**：fail-open 违反 P0-A 契约，写进 ADR 即等同于把红线外推。

---

## 6. D4 — Fail-closed 语义

### 6.1 问题分解

#127 D4 原文：「沙箱不可用时显示什么错误信息？是完全禁用 shell 还是只禁用某些命令？」

需要分两个子问题：

- **D4a**：沙箱**初始化失败**（`OsExecutor::new` 报错、`start_network_proxy` 启动失败）时怎么办？
- **D4b**：沙箱**运行时拦截**（命令访问非允许域名、写非允许路径）时怎么办？

### 6.2 D4a 候选

| 选项 | 描述 | 风险 |
|---|---|---|
| **A. App 启动失败**（panic）→ 用户必须修复 config | 最严格 fail-closed | desktop UX 灾难 |
| **B. App 启动成功但 ShellTool 不注册**（dev tools 缺失） | enterprise mode 下用户能开 app，但 shell 工具不可见 | 与 P0-B (#84) "tool visibility triple gate" 自然契合 |
| **C. App 启动成功，ShellTool 注册但 `execute()` 永久返回错误** | UI 上看到 shell tool 但每次调用都失败 | 误导用户 |
| **D. App 启动成功，silent fall-back 到 direct executor** | fail-OPEN | **直接淘汰** |

### 6.3 D4b 候选

OS 沙箱 + proxy 拦截已有的成熟语义（参考 [`crates/dasclaw_net_proxy/src/reasons.rs`](../../../crates/dasclaw_net_proxy/src/reasons.rs) `NetworkDenyReason`）：

- 网络访问被拒 → 返回结构化 error，UI 可显示 "domain.example.com 不在允许列表"
- 文件写入被拒 → seatbelt/landlock 自然报错，stderr 含 EACCES

**这部分不需要新决策，沿用已有实现即可。**

### 6.4 Trade-off

> **🟢 agent 建议**：
>
> **D4a**：**选项 B（enterprise mode 下沙箱不可用 → ShellTool 不注册）**。
>
> 理由：(1) 与 P0-B [#84](https://github.com/Linnanli/xClaw/issues/84) "tool visibility triple gate" 契合；(2) UX 体验是"工具不可见"而不是"工具可见但永远报错"；(3) 个人开发者模式下沙箱失败回退到 direct（仍 silent fall-back，但因 D1=C 已限定不在 enterprise mode），不违反 fail-closed 契约。
>
> **D4b**：沿用现有 `NetworkDenyReason` + seatbelt/landlock 自然错误，**无需新决策**，仅需 ADR 中明文记录"沿用既有实现"。

---

## 7. D5 — Default `SandboxPolicy`（沙箱激活时）

### 7.1 现状

- `ShellTool::new()` 默认 `sandbox_policy: SandboxPolicy::ReadOnly`（`Default for SandboxPolicy` 也是 `ReadOnly`）
- W3.2b 完成报告 [`44-net-proxy-port-completion.md`](./44-net-proxy-port-completion.md) caller 示例使用 `WorkspaceWrite`
- codex 测试默认 `WorkspaceWrite`（参考 §0.4）

### 7.2 候选

| 选项 | 文件 | 网络 | 适用场景 |
|---|---|---|---|
| `ReadOnly` | `/workspace` 只读 | 经 proxy | 只读勘探（curl docs / cat 配置） |
| `WorkspaceWrite` | `/workspace` 读写 | 经 proxy | 改代码、跑测试、写文件 |
| `FullAccess` | 全宿主 | 直连 | **必须双 opt-in**，ADR 不应作为默认 |

### 7.3 Trade-off

- `ReadOnly` 太严格 → 用户首次跑 `cargo build` 就被拒绝，体验差
- `WorkspaceWrite` 是 codex 上游默认 + W3.2b doc 示例默认 → 工程惯例
- `FullAccess` 已有 `SANDBOX_ALLOW_FULL_ACCESS` double-opt-in，永远不应默认

> **🟢 agent 建议**：**`WorkspaceWrite`**。
>
> 理由：(1) 与 codex 默认对齐；(2) 与 W3.2b-5 caller pattern 默认对齐；(3) `ReadOnly` 在 desktop dev 场景过于受限；(4) 网络仍走 proxy → 安全姿态足够；(5) `FullAccess` 仍需 double-opt-in，不会被误启用。
>
> **配套约束**：ADR 必须明文写「`policy: FullAccess` 在 enterprise mode 下被忽略并降级到 `WorkspaceWrite`」（已有 [`SandboxModeConfig.allow_full_access` 降级逻辑](../../../desktop-client/ironclaw/src/config/sandbox.rs)，但 OS sandbox 路径需要等价机制）。

---

## 8. D6 — Proxy 启用条件

### 8.1 候选

| 选项 | 描述 |
|---|---|
| **A. proxy 与 OS 沙箱共开关**（`os_sandbox.enabled` 同时控制两者） | 简单 |
| **B. 独立 `os_sandbox.network_proxy.enabled` toggle** | proxy 可在沙箱关闭时单独启用 |
| **C. proxy 总是开**（无论沙箱开关） | 最严格 |

### 8.2 codex 对照

codex `Session::start_managed_network_proxy` 是无条件启动的（参考 [`44-net-proxy-port-completion.md`](./44-net-proxy-port-completion.md)），没有 toggle。

### 8.3 Trade-off

- 选项 B 增加用户配置复杂度，但提供了"我只想要 DLP 不想要文件沙箱"的能力 → 与 P0-F [#92](https://github.com/Linnanli/xClaw/issues/92) "Attachment DLP gate" 自然契合
- 选项 A 简单但牺牲了"只用 proxy 不用沙箱"的中间档

> **🟢 agent 建议**：**选项 A（共开关），但留 `network_proxy.always_on: bool` 后门**。
>
> 理由：(1) D1 已经把"是否启用沙箱"和"是否 enterprise mode"绑定，再加一层独立 proxy toggle 让组合矩阵从 2 维变 4 维，违反 [`AGENTS.md`](../../../AGENTS.md) "禁止补丁式代码"；(2) `always_on: bool` 后门给未来 P0-F (#92) DLP 路径留出口；(3) 默认值 `always_on = false`，第一版不暴露给用户。

---

## 9. D7 — 迁移窗口

### 9.1 现状

- `SandboxModeConfig.enabled` 默认 `true`（Docker mode），`env: SANDBOX_ENABLED` 现有用户可能已显式设置
- 假设 D0 = 选项 B（独立命名空间），那么新 `os_sandbox.enabled` 是新字段，**默认值不冲突**

### 9.2 候选

| 选项 | 描述 |
|---|---|
| **A. 不需要迁移**（D0=B 自然兼容） | 最简单 |
| **B. 强制重写迁移**（`SANDBOX_ENABLED` 同时影响 Docker 和 OS sandbox） | 破坏向后兼容 |
| **C. 软迁移**（启动时检测旧 `SANDBOX_ENABLED` 并日志警告） | 中庸 |

### 9.3 Trade-off

> **🟢 agent 建议**：**选项 A（不需要迁移）**。
>
> 理由：D0=B 后两个命名空间独立，旧用户配置 `SANDBOX_ENABLED=true` 仍只控制 Docker mode；新用户启用 OS sandbox 需要显式设置 `OS_SANDBOX_ENABLED=true`（或 admin policy 推送）→ **零迁移成本**。
>
> 若决策者选 D0=A（复用），则迁移决策必须重新评估，本草稿的 D7 建议作废。

---

## 10. 决策汇总（agent 建议矩阵）

> ⚠️ 以下是 **agent 建议**，**不是 ADR Accepted 决策**。最终需 [#127](https://github.com/Linnanli/xClaw/issues/127) 决策人在 ADR 文件中签字。

| # | 决策点 | agent 建议选项 | 关键理由 |
|---|---|---|---|
| **D0** | 字段命名空间 | **B — 独立 `os_sandbox.*`** | 最小破坏 + 语义清晰，避免与 Docker mode `SandboxModeConfig` 糅合 |
| **D1** | 默认 `enabled` | **C — 默认 `false`，enterprise mode 强制 `true`** | 保护现有用户体验 + 满足 P0-A enterprise fail-closed 契约 |
| **D2** | Rollout 策略 | **E + C — enterprise 强制 + 个人模式 release notes opt-in** | 与 #95 enterprise policy 契合 |
| **D3** | Windows 行为 | **D — enterprise 强制走 Docker mode**（B 兜底） | 不阻塞在 #91 上；Docker 路径已成熟 |
| **D4** | Fail-closed 语义 | **B — 沙箱不可用时 ShellTool 不注册** | 与 P0-B (#84) tool visibility 契合 |
| **D5** | 默认 `SandboxPolicy` | **`WorkspaceWrite`** | 与 codex 上游 + W3.2b-5 doc 默认对齐 |
| **D6** | Proxy 启用 | **A — 共开关 + `always_on` 后门** | 复杂度最低，给 P0-F (#92) DLP 留出口 |
| **D7** | 迁移窗口 | **A — 不需迁移** | D0=B 后两命名空间独立 |

---

## 11. 与其他红线 issue 的交叉影响

本决策若按 §10 矩阵落地，对其他红线 issue 的影响：

| 红线 | 受影响方式 |
|---|---|
| **#28** 父 issue | 本决策 = #28 "fail-closed 契约"具体化；ADR 落地后 #28 可关闭 |
| **#84** P0-B tool visibility triple gate | D4 选项 B 直接帮 #84 减少一层语义负担（沙箱不可用 = tool 不可见，复用同一 gate） |
| **#85** P0-D enterprise tool audit | D1 选项 C 让 audit 路径只需要审 enterprise mode；D7 不迁移让 audit 范围 smaller |
| **#91** P1 Windows sandbox 计划 | D3 选项 D 把 #91 从 P1 必须项**降级为可延后**（enterprise Windows 走 Docker，不再 block） |
| **#92** P0-F attachment DLP gate | D6 选项 A 的 `always_on` 后门留给 #92 在不打开 OS 沙箱时单独启用 proxy |
| **#94** P0-H tool execution surface parity | D4 选项 B "ShellTool 不注册" 把 desktop / job / routine / container 四个 surface 的 fail-closed 语义统一到「不可见」 |
| **#95** P0-I enterprise tenant fail-closed | D1 选项 C 的 enterprise 强制开关 = #95 admin policy 推送的一个具体应用 |
| **#96** P0-J dynamic capability governance | D6 `always_on: bool` 后门是 #96 future 治理面板的一个 capability 字段 |
| **#127** 本决策 ADR | 自身 |
| **#128** 实现 issue | 落定后解锁 #128：a) 新增 `OsSandboxConfig` struct；b) `BootstrapContext` 新增 sandbox/proxy 字段；c) `register_dev_tools` 按 D4 选项 B 拆分 |

---

## 12. 非目标（建议在 ADR Non-goals 段显式记录）

避免后续 ADR 被 PR 反复扩张，建议把以下项目在 ADR Non-goals 明文标记 deferred：

1. ❌ Windows OS 沙箱 backend 实现 — 留给 #91（若 D3=B/D 不需要再做）
2. ❌ `ExecutionMode` 三值 enum 重构 — 留给 W4+ 单独 ADR（D0 选项 C）
3. ❌ DLP / proxy 单独面板 toggle — 留给 #92 / #96 dynamic capability governance
4. ❌ Sandbox-on E2E 测试 CI 集成 — 留给 #128 实现 PR 自身（不算决策项）
5. ❌ `extra_env` 合并 vs 覆盖语义重新设计 — [inventory §11.9](./p0a-sandbox-activation-inventory.md) 已确认现有"caller 优先"语义，沿用即可，不需 ADR
6. ❌ `SandboxConfig` ↔ `SandboxModeConfig` 双类型合并 — 留给 D0 选项 C 的未来 ADR
7. ❌ Personal mode 的 sandbox UX（首次启动 prompt / wizard） — 留给独立 UX issue（与红线无关）

---

## 13. 决策后 #128 实现指引（一旦 ADR Accepted 即可启动）

> 以下是**预期实施清单**，给 #128 PR 作者参考。**本草稿不约束 #128 acceptance**，仅作 hand-off 材料。

按 §10 矩阵落地，#128 实现 PR 应包含：

1. **新增 [`config/os_sandbox.rs`](../../../desktop-client/ironclaw/src/config/) `OsSandboxConfig` struct**
   - 字段：`enabled: bool`（默认 `false`），`policy: SandboxPolicy`（默认 `WorkspaceWrite`），`network_allowlist: Vec<String>`，`proxy_port: u16`，`network_proxy: NetworkProxyConfig { always_on: bool }`
   - `resolve(settings, is_enterprise_mode)` 在 enterprise mode 下强制 `enabled = true`、`allow_full_access = false`
2. **修改 [`tools/bootstrap.rs::BootstrapContext`](../../../desktop-client/ironclaw/src/tools/bootstrap.rs)**
   - 新增 `pub os_sandbox_executor: Option<Arc<OsExecutor>>`
   - 新增 `pub network_proxy_handle: Option<Arc<NetworkProxyHandle>>`
   - 新增 `pub proxy_env: Option<HashMap<String, String>>`
3. **修改 [`tools/registry.rs::register_dev_tools`](../../../desktop-client/ironclaw/src/tools/registry.rs)**
   - 不再 silent 回退到 direct executor
   - 按 D4 选项 B：if `is_enterprise && os_sandbox_executor.is_none()` → **跳过注册** + 日志 ERROR
   - else if `os_sandbox_executor.is_some()` → 注册 `ShellTool::new().with_sandbox(...).with_sandbox_policy(...).with_extra_env(...)`
   - else → 保持现有 direct 行为（个人模式 fall-back）
4. **修改 [`app.rs`](../../../desktop-client/ironclaw/src/app.rs) boot 序列**
   - `init_secrets()` 之后、`bootstrap_tools()` 之前
   - 若 `os_sandbox.enabled` → 构造 `OsExecutor` + 启动 `start_network_proxy`，存入 `BootstrapContext`
   - `NetworkProxyHandle` 存入 orchestrator 长生命周期字段
5. **测试**（参考 [`AGENTS.md`](../../../AGENTS.md) 测试矩阵）：
   - 单元：`OsSandboxConfig::resolve` 在 enterprise mode 下的 fail-closed 行为
   - 失败路径：`OsExecutor::new` 失败时 enterprise mode → ShellTool 不注册（D4 选项 B 契约）
   - 集成：[`integration_smoke_tests.rs`](../../../admin-backend/tests/integration_smoke_tests.rs) 等价的 desktop-client 启动冒烟
   - E2E：沙箱 on + 命中非允许域名 → 拒绝；命中允许域名 → 通过
   - 安全审计：日志中不泄露 `proxy_env` 中的 `Bearer` token
6. **文档**：`desktop-client/docs/os-sandbox-activation.md`（D2 release notes 引导）

---

## 14. 三层验证日志

按 [`AGENTS.md`](../../../AGENTS.md) §"任务启动 4 问"：

1. **新增文件**？✅ 是 → 已 `semantic_search` 验证 `docs/plans/architecture-refactor/p0a-sandbox-activation-decision*.md` 不存在
2. **否定性结论**？✅ 是（"OS 沙箱 0 生产调用方"、"无 Windows backend"）→ 证据全部通过 [`p0a-sandbox-activation-inventory.md`](./p0a-sandbox-activation-inventory.md) 的 Level 1+3 grep 引证
3. **跨项目对账**？✅ 是（codex `SandboxPolicy::new_workspace_write_policy` / `start_managed_network_proxy`）→ 已 `grep_search` 验证 codex 测试 + 生产路径
4. **架构对账文档**？✅ 是 → 三层验证已应用于每条建议

**Level 2 (vscode_listCodeUsages) 限制**：与 inventory 一致，Rust 不支持，已用第二条 Level 3 grep 替代。

---

## 15. 用法说明

### 给 [#127](https://github.com/Linnanli/xClaw/issues/127) 决策人

1. 阅读本文件 + [`p0a-sandbox-activation-inventory.md`](./p0a-sandbox-activation-inventory.md)
2. 对 §10 矩阵的 8 项（D0–D7）逐项打勾或修改
3. 在 `docs/plans/architecture-refactor/adr-XXX-p0a-sandbox-activation-decision.md` 创建正式 ADR（按 [#127 acceptance](https://github.com/Linnanli/xClaw/issues/127) 要求 Status=Accepted 签字）
4. ADR Accepted 后关闭 [#127](https://github.com/Linnanli/xClaw/issues/127)，解锁 [#128](https://github.com/Linnanli/xClaw/issues/128)
5. 若 ADR 实质偏离本草稿（如 D0 选 A 或 D3 选 A），**作废本草稿 §13 实施指引**，由 #128 PR 作者重写

### 给 [#128](https://github.com/Linnanli/xClaw/issues/128) 实现者

- **不要把本草稿当 ADR 引用**——本草稿没有人类签字
- ADR Accepted 后，按 ADR 而非本草稿实施
- 若 ADR 与本草稿差异大，§13 实施指引可能完全失效

### 给后续会话 / agent

- 本草稿是**研究材料**，不是 spec
- 本草稿不应被 git diff 视为"激活了 sandbox"——它只产生 markdown，无 Rust 改动
- 若 ADR 决策与本草稿一致，本草稿可作为 #128 PR 的 hand-off 资料引用
- 若 ADR 决策与本草稿不一致，本草稿应在 ADR Accepted 后**移动到 `docs/plans/archive/`** 或删除

---

## 附录 A — codex Windows sandbox 实地考察（事实修正）

> 本附录由会话内 D3 决策深挖时增补，**修正本草稿正文（含 §1 / §6 / §13）多处关于 "codex 上游不做 Windows sandbox / `UnsupportedWindowsSandbox`" 的判断**。后续 ADR 应基于本附录的事实重写 D3 章节。

### A.1 事实修正

之前的研究仅检查 [`codex-cli-main/codex-rs/sandboxing/src/`](../../../codex-cli-main/codex-rs/sandboxing/src/) 目录，未发现 Windows 实现，**错误推断 codex 没有 Windows 沙箱**。重新做 [`list_dir`](../../../codex-cli-main/codex-rs/) 后发现：

- codex 的 Windows 沙箱实现在**独立 crate** [`codex-cli-main/codex-rs/windows-sandbox-rs/`](../../../codex-cli-main/codex-rs/windows-sandbox-rs/)（package name `codex-windows-sandbox`）
- 该 crate 约 30 个 `.rs` 模块，**估计 5000+ LOC**
- 该 crate 提供 2 个 binary：`codex-windows-sandbox-setup`（要求 UAC 提权做一次性 setup）+ `codex-command-runner`（运行时执行沙箱命令）
- License: codex 整仓 **Apache 2.0**

错误根因：违反 [`AGENTS.md`](../../../AGENTS.md) §"分析工具使用规范" 三层验证原则——只看 `sandboxing/src/` 子目录就断言"codex 没做 Windows 沙箱"，未对 workspace 其他 crate 做 `list_dir`，是典型字面量搜索陷阱。

### A.2 codex 的 Windows 沙箱真实策略

**不走 AppContainer，走"古典 Windows 多用户隔离"路线**：

| 模块 | 功能 |
|---|---|
| [`sandbox_users.rs`](../../../codex-cli-main/codex-rs/windows-sandbox-rs/src/elevated/sandbox_users.rs) | **创建专用本地 Windows 用户**（`CodexSandboxUsers` 组下 offline + online 两用户，密码随机 + DPAPI 加密存储） |
| `token.rs` | Restricted Token（CreateRestrictedToken） |
| `cap.rs` | 自定义 `S-1-5-21-XXX` SID 风格做工作区隔离 marker（**不是真正的 AppContainer capability SID**，AppContainer 是 `S-1-15-3-` 前缀） |
| `acl.rs` / `workspace_acl.rs` / `audit.rs` | DACL / world-writable 扫描 / deny ACE |
| `firewall.rs` | **按用户 SID 设 Windows Firewall 规则**（INetFwPolicy2） |
| `desktop.rs` | **Win32 alternate desktop**（StationsAndDesktops，独立桌面隔离） |
| `dpapi.rs` | DPAPI 加密凭据 |
| `hide_users.rs` | 隐藏沙箱用户 profile dir |
| `conpty/` | **ConPTY 伪终端**（让交互式命令能跑） |
| `setup_orchestrator.rs` + `elevated/` | UAC setup helper（一次性创建用户/组） |
| `proc_thread_attr.rs` | STARTUPINFOEXW + ProcThreadAttributeList |
| `process.rs` / `spawn_prep.rs` / `unified_exec/` | 进程管理 |

**依赖**（[`Cargo.toml`](../../../codex-cli-main/codex-rs/windows-sandbox-rs/Cargo.toml)）：

```toml
windows = "0.58"            # Foundation, Firewall, COM
windows-sys = "0.52"        # JobObjects, Authorization, Threading, Security,
                            # NetManagement, StationsAndDesktops, Registry, ...
```

**官方 windows / windows-sys crate 直写，无第三方 sandbox lib**（无 `firehazard` / 无 `rappct` / 无 `windows-app-isolation` 等）。

### A.3 Windows 版本兼容性下限

| API | 最低 Windows |
|---|---|
| Job Object / Restricted Token / DPAPI / CreateProcessAsUserW | Windows 2000 |
| Window Stations / alternate desktop | Windows NT 3.1 |
| NetUserAdd / NetLocalGroupAdd | Windows NT 3.5 |
| INetFwPolicy2 | Windows Vista（Win7 起完整稳定） |
| STARTUPINFOEXW + ProcThreadAttributeList | Windows Vista |
| **CreatePseudoConsole（ConPTY）** | **Windows 10 1809 / Build 17763 / 2018-10** ⚠️ 硬约束 |

**唯一卡死下限：ConPTY = Win10 1809**。若放弃交互式 PTY 命令，可下探 Win7；但 AI 助手需要 PTY，所以**实际下限 = Win10 1809 / Server 2019**。

### A.4 政企客群覆盖率

| Windows 版本 | EOS | codex sandbox 支持 | 政企份额 |
|---|---|---|---|
| Windows 7 | 2020-01 | ❌ | 残留 1-3% |
| Windows 8 / 8.1 | 2023-01 | ❌ | <1% |
| Windows 10 1607~1803 | 已 EOS | ❌ | <0.5% |
| **Windows 10 1809（LTSC 2019）** | 2029（LTSC） | ✅ | 政企广泛 |
| **Windows 10 21H2（LTSC 2021）** | 2032（LTSC） | ✅ | 政企广泛 |
| **Windows 11 22H2+** | 2032+ | ✅ | 增长中 |
| **Windows Server 2019 / 2022** | 2029 / 2031 | ✅ | 政企服务器主力 |

**政企客户端实际覆盖：99%+**。LTSC 2019 / LTSC 2021 / Win11 / Server 2019+ 全在支持范围。

### A.5 codex 用户隔离方案 vs AppContainer 路线对比

| 维度 | 5++（AppContainer + JO + RT，自写 ~1500 LOC） | **codex（用户隔离 + JO + RT + Firewall + Desktop，~5000 LOC）** |
|---|---|---|
| 核心隔离机制 | AppContainer SID + capability | **专用 Windows 用户身份** |
| 隔离哲学 | 现代 capability-based | 古典 NT 多用户模型（30 年历史） |
| AV / 域 GPO 兼容 | ⚠️ 中（AppContainer 与 AV 冲突 5 类，详见 §6.X 之前研究） | **✅ 高（用户隔离 AV/域都认） |
| 第三方工具兼容（PowerShell/oracle/SSH/JDK） | ⚠️ AC 内常挂 | **✅ 不同用户身份正常跑全部工具** |
| 工程量（自写） | ~1500 LOC | ~5000 LOC（fork 后整合 ~500 LOC） |
| 首次启动 UX | 直接启动 | **首次需 UAC setup**（创建用户/组） |
| 后续启动 UX | 直接 | 直接（setup 一次性完成） |
| 隔离强度 | 中（capability） | **高（独立 SID + 独立 ACL + 独立 desktop）** |
| GUI 程序兼容 | ⚠️ 部分挂 | ✅ ConPTY 转发 |
| 凭据保护 | N/A | DPAPI 加密 |
| 网络隔离 | capability + Firewall | 按用户 SID Firewall |
| 是否踩 AV 兼容性深坑 | ⚠️ 是 | **✅ 否** |
| 生产验证 | 无（待真机矩阵测试） | **✅ OpenAI Codex CLI 生产使用** |
| License | N/A（自写） | **Apache 2.0（可 fork / vendor）** |

### A.6 推荐方案 X — Fork codex-windows-sandbox

**核心动作**：把 `codex-cli-main/codex-rs/windows-sandbox-rs/` fork 进 x-claw monorepo，作为 `crates/dasclaw_sandbox_windows`（或并入 `crates/dasclaw_sandbox` 的 windows 子模块）。

| 阶段 | 内容 | LOC |
|---|---|---|
| W3 | Fork + 适配 dasclaw_sandbox 接口 + 改名（`CodexSandboxUsers` → `DasclawSandboxUsers`、binary 名等） + 添加 Apache-2.0 NOTICE attribution | ~500（整合工作） |
| W3-W4 | 真机矩阵（Win10 1809 / 21H2 / Win11 / Server 2019 / 域机器 / 杀软 SCEP/Defender/360/瑞星等）+ bug fix + setup UAC 体验文档 | +500 |
| 长期 | 跟踪 upstream codex 关键修复，cherry-pick 必要变更 | 低维护成本 |

**对比矩阵**：

| 方案 | LOC（自写） | 兼容性 | 风险 |
|---|---|---|---|
| 5++（AppContainer 自写） | 1500 | 中（AV 踩坑） | 中 |
| **X（fork codex）** | **500（整合）** | **高（生产验证）** | **低** |

**注意事项**：
1. 首次 UAC setup 弹窗 → 政企部署文档明确写"IT 一次性运行 setup.exe"，不是终端用户日常操作
2. License attribution → NOTICE / README 标注 "derived from openai/codex (Apache-2.0)"
3. 品牌改名：`CodexSandboxUsers` → `DasclawSandboxUsers`、`codex_home` → `dasclaw_home`、binary 名前缀替换
4. 协议适配：codex 的 `SandboxPolicy` 与 ironclaw `SandboxPolicy` 类型不同，需协议层 adapter
5. 依赖剥离：codex-windows-sandbox 依赖 `codex_protocol` / `codex_utils_pty` 等，需要决定 fork 哪些子 crate / 用 ironclaw 等价替代

### A.7 对 D3 决策的影响（重写要求）

本草稿正文 §1 D3 的 4 选项（A=不做 / B=Docker / C=自写 OS sandbox / D=Docker only on Windows）**均与新事实不符**，应在 ADR 中重写为：

| 新 D3 选项 | 内容 | 推荐度 |
|---|---|---|
| D3-1 | Windows 走 Docker（原 D 选项） | 弱（破坏 Case A 政企"本地 Word/Excel 无缝访问") |
| D3-2 | 自写 AppContainer + JO + RT（原 5++） | 中（1500 LOC 但 AV 踩坑风险） |
| D3-3 | **Fork codex-windows-sandbox（方案 X）** | **强（500 LOC 整合 + 生产验证 + AV 兼容)** |
| D3-4 | 推迟到 W4 / W5 + 临时 Permission-only | 弱（政企招标硬约束需要技术沙箱） |

**Agent 推荐：D3 = D3-3（方案 X / fork codex-windows-sandbox）**。

理由：
1. OpenAI 团队投入 5000 LOC 走用户隔离而非 1500 LOC 走 AppContainer，说明 AppContainer 在政企真实环境（域+AV）确有问题，不是过度担心
2. Apache 2.0 license 合法 fork，5000 LOC 现成代码省去 3 倍工程量
3. 用户隔离对 AV / 域 GPO / PowerShell / oracle / SSH / JDK 等政企常见工具兼容性显著优于 AppContainer
4. 生产验证（OpenAI Codex CLI 在用），比从头写更稳
5. Win10 1809+ 兼容性匹配政企客群（99%+ 覆盖）
6. 长期维护可跟 upstream cherry-pick

### A.8 后续 issue / ADR 提案

ADR 作者拍板 D3 = D3-3 后，应：

1. 在 W3 issue 链路下新增 epic issue：**"Fork codex-windows-sandbox into dasclaw monorepo"**（具体 issue draft 见 [`p0a-sandbox-fork-codex-windows-sandbox-issue-draft.md`](./p0a-sandbox-fork-codex-windows-sandbox-issue-draft.md)）
2. ADR 中写明依赖：W3 #128 的 Windows 实现路径选择 D3-3 时，自动绑定该 fork epic
3. 评估是否需要同时 fork codex 上游的 `codex-utils-pty` / `codex-protocol`（依赖项）或用 ironclaw 等价替换

---
