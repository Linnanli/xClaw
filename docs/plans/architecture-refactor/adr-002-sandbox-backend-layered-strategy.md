# ADR-002：沙箱后端分层策略（cap-std 应用层 + Codex 模式内核层）

- **状态**：已采纳 2026-04-23
- **适用范围**：`crates/ironclaw_sandbox_*`、`crates/x_claw_agent::SandboxExecutor`、`desktop-client/ironclaw/src/sandbox/`、Phase 3 后期 & Phase 4 整体
- **前置**：[ADR-001（Phase 3 不接进程内 sandbox hook）](./adr-001-sandbox-hook-not-wired-in-phase3.md)

## 背景

原 Phase 3 Step E 假设自研沙箱；ADR-001 重新对齐上游后，Phase 4 要落真实沙箱。决策前评估了 4 条候选路径：

| 路径 | Linux | macOS | Windows | 问题 |
|---|---|---|---|---|
| A 纯 Docker | Docker | Docker | Docker | 桌面 macOS/Windows 用户必须装 Docker Desktop，政企接受度差 |
| B 纯 unshare（claw-code） | 手写 | ❌ | ❌ | macOS/Windows 不支持；文件只做 env 级软隔离 |
| C unshare + Docker 可选 | 手写 | ❌ | ❌ | macOS 还是没保护；依然依赖 Docker |
| **D+ 平台原生分层 + 成熟库** | **extrasafe** / Codex linux-sandbox | Codex seatbelt | Codex windows-sandbox | 代码量可控，跨平台原生 |

OpenAI Codex CLI（Apache-2.0，`openai/codex`）是业内唯一开源的**三平台原生沙箱 Rust 实现**，代码已在生产跑（77k ★，0.124 附近高频 release），本地已作为 submodule 挂到 `codex-cli-main/`。

另外 `cap-std`（Bytecode Alliance，WASI/Deno 在用）解决了**应用层 Rust 代码自己读写文件时**的路径绕过问题（符号链接、`..`、TOCTOU），补位 Codex SandboxPolicy 不覆盖的同进程场景。

## 决策

### 决策 1 — 采用 **D+ 路径**（分层原生沙箱 + 成熟库）

Phase 4 沙箱后端按三层堆叠：

```
┌─────────────────────────────────────────────────────────┐
│ 兜底层  ironclaw_safety                                 │
│          DLP / 审批 / 工具白名单 / 路径策略             │
│          永远启用，所有平台                              │
├─────────────────────────────────────────────────────────┤
│ 应用层  cap-std                                         │
│          宿主 Rust 进程所有 FS 访问经 Dir capability    │
│          防我们自己代码误用（TOCTOU / 符号链接 / ..）   │
│          所有平台                                        │
├─────────────────────────────────────────────────────────┤
│ 内核层  Codex 模式三平台沙箱                            │
│          约束 LLM 生成的子进程（bash/代码/工具）        │
│          Linux   : Landlock + seccomp                   │
│          macOS   : sandbox-exec (SBPL)                  │
│          Windows : Restricted Token + Job Object        │
├─────────────────────────────────────────────────────────┤
│ 可选     Docker daemon 后端                             │
│          企业/高风险场景；对齐上游 ironclaw engine v2   │
└─────────────────────────────────────────────────────────┘
```

**cap-std 和 Codex 沙箱不是二选一，是叠加**：

| 威胁 | 由谁防 |
|---|---|
| 宿主 Rust 代码误用文件路径（TOCTOU / 符号链接 / `..`） | **cap-std** |
| LLM 子进程访问 workspace 外文件 | **Codex 沙箱** |
| LLM 子进程网络泄漏 | **Codex 沙箱** |
| LLM 子进程资源 DoS | **Codex 沙箱**（cgroup / Job Object） |
| 模式已知的危险命令 / 敏感数据 | **ironclaw_safety** |

### 决策 2 — **fork Codex 子 crate 为起点**，改名 `crates/ironclaw_sandbox_*`

Codex 是 Apache-2.0，允许 fork + 修改分发（需保留版权声明 + NOTICE）。fork 目标：

| 来源（codex-cli-main/codex-rs/） | 目标（本仓库 crates/） | 范围 |
|---|---|---|
| `core/src/landlock.rs` + `linux-sandbox/` bin + `sandboxing/src/...` Linux 相关 | `ironclaw_sandbox_linux` | Landlock + seccomp + `codex-linux-sandbox` 辅助 binary |
| `sandboxing/src/seatbelt.rs` + SBPL 策略模板 | `ironclaw_sandbox_macos` | `Command::new("sandbox-exec")` 调用 + 策略动态生成 |
| `windows-sandbox-rs/` 整个子 crate | `ironclaw_sandbox_windows` | Restricted Token + Job Object + ConPTY + 命名管道 IPC + `codex-command-runner.exe` 辅助 binary |
| `sandboxing/src/manager.rs` 的 `SandboxManager::transform` 抽象 | `ironclaw_sandbox_common` | 跨平台 `SandboxPolicy` / `SandboxExecRequest` / `SandboxType` |

**不 fork 的部分**：`core/src/sandboxing/mod.rs` 里的 Codex agent 专有逻辑（`Session`、`TurnContext` 等）；这些由我们的 `x_claw_agent::SandboxExecutor` trait 替代承接。

**替代方案（未采用）**：
- Linux 用 `extrasafe` 而非 fork Codex — 放弃，理由：保持三平台**统一参考来源**，减少心智负担；extrasafe 仅 x86_64，Codex Linux 实现支持更广（aarch64 也覆盖）
- 从零重写 — 放弃，理由：Windows 沙箱 ACL/Token 编程 5-10 KLOC，自己写容易漏安全细节

### 决策 3 — **cap-std 作为独立 Phase 3 子任务**（不阻塞沙箱）

- 工作量：半天到一天
- 收益：立即堵上应用层 FS 边界所有已知绕过
- 改动面：`desktop-client/ironclaw/src/sandbox/agent_executor.rs::resolve_path_bounded` + `workspace_dir.rs`
- 与 Codex 沙箱**并行推进**

### 决策 4 — **Codex fork 按 Linux → macOS → Windows 顺序分阶段**

- Linux 先做：Landlock 成熟、测试环境好搭（CI Ubuntu runner 天然支持）
- macOS 次：sandbox-exec 代码量最小（主要是 SBPL 策略设计）
- Windows 最后：代码量最大（~5-10 KLOC）、测试环境最复杂，但也是政企客户主力，不能跳

### 决策 5 — **Docker daemon 后端定位为"可选强化"**

对应 Phase 5 或更晚：
- 触发条件：企业客户显式要求 / 本机缺内核级沙箱能力（如老内核 Linux）
- 对齐上游 ironclaw engine v2 + `bridge/sandbox/` + `sandbox_daemon` binary
- 不作为默认路径

## 理由

| 考量 | D+ 方案结果 |
|---|---|
| **跨平台一致** | 三平台都有内核级保护（Codex 模式），不像 C 路径 macOS/Windows 裸跑 |
| **免强依赖** | 不强制用户装 Docker，桌面端开箱即用 |
| **政企兼容** | Windows Vista+ 全覆盖（Restricted Token 档位免管理员权限） |
| **性能** | 5-20ms spawn 开销，运行时零成本，比 Docker 快 10× |
| **代码质量** | fork 自 Codex 生产级代码，不自己从零写 |
| **维护成本** | Codex 上游活跃，关键安全修复可反向 cherry-pick |
| **对齐 AGENTS.md "优先成熟库"** | cap-std（WASI 标准依赖）+ Codex fork（Apache-2.0）均满足 |

## 对齐 AGENTS.md 原则

- **优先成熟社区库**：cap-std 直接使用；Codex 以 fork 形式使用（含完整许可证追踪）
- **TDD**：每个后端 crate 必须有：契约测试（实现 `SandboxExecutor` trait） + 失败路径测试（策略违规拒绝） + 安全审计测试（逃逸尝试） + 集成测试（真实沙箱环境）
- **Fail-Safe**：沙箱失败时拒绝执行而非裸跑。fallback 链：Codex 沙箱 → `ironclaw_safety` 审批 → 拒绝
- **多道防线**：即使内核层被绕过，应用层 cap-std + 兜底层 ironclaw_safety 仍有效

## 后续动作

1. **Phase 3 尾期**：落地 cap-std 工作区重构（E-cap-std，见 05b 执行计划更新）
2. **Phase 4 初**：建 `crates/ironclaw_sandbox_common` + trait 实现 skeleton（E-sandbox-trait）
3. **Phase 4 中**：按 Linux → macOS → Windows 顺序 fork Codex 对应子模块
4. **Phase 4 末 / Phase 5**：评估 Docker daemon 后端是否需要（企业客户驱动）
5. **持续**：跟进 Codex 上游安全修复，定期（季度级）rebase / cherry-pick
6. **许可证合规**：`LICENSES/codex-NOTICE.md` 记录 fork 来源 + commit hash + Apache-2.0 文本副本

## 与 ADR-001 的关系

- ADR-001：决定 Phase 3 **不接** 进程内 sandbox hook，保留 `SandboxExecutor` trait 作可选契约
- ADR-002（本文）：定义 Phase 4 如何**接**沙箱 —— 通过 fork Codex 三平台实现填充 `SandboxExecutor` trait，搭配 cap-std 应用层加固

两者连贯：ADR-001 说"先不接、留契约"，ADR-002 说"后面这么接"。

## 参考

- 本仓库：[docs/plans/architecture-refactor/adr-001-sandbox-hook-not-wired-in-phase3.md](./adr-001-sandbox-hook-not-wired-in-phase3.md)
- 本仓库：[docs/plans/architecture-refactor/05b-phase3-execution-plan.md](./05b-phase3-execution-plan.md)
- 上游参考（submodule）：[codex-cli-main/](../../../codex-cli-main/)（`openai/codex` @ `d3b044938`）
- 上游参考（下载副本，.gitignored）：`ironclaw-main/`（`nearai/ironclaw` HEAD）
- cap-std：<https://docs.rs/cap-std>
- Codex Apache-2.0：<https://github.com/openai/codex/blob/main/LICENSE>
