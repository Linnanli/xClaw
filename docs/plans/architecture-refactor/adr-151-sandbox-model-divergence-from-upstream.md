# ADR-151 — Sandbox 模型与 claude-code 上游的架构分歧

- **状态**：Accepted
- **作用域**：`crates/dasclaw_sandbox*` / `desktop-client/ironclaw/src/sandbox/*` / `desktop-client/ironclaw/src/orchestrator/job_manager.rs`
- **依赖**：ADR-002（sandbox backend 分层）、ADR-121（P0A sandbox 激活决策）、ADR-119（desktop-client job runtime）、ADR-041（Docker vs OS sandbox 能力对比）
- **触发上游**：`claude-code-main/src/tools/BashTool/shouldUseSandbox.ts`（153 LOC）+ `SandboxManager`
- **关联 epic**：#490（bash-validation parity），#608（Phase 2.3 Ask UX），#607（Phase 4.2 sed 严格白名单 hook 接入）
- **关联 PR**：#606（Phase 4.1 sed AST 完成）

---

## 1. 背景

在 epic #490（bash-validation parity）收尾盘点中发现：上游 `claude-code-main` 的 BashTool 在
`shouldUseSandbox.ts` 实现了 **per-command 沙箱决策**，对每条要执行的 bash 命令独立判定
"该不该进沙箱"。这套机制 153 LOC，依赖 `SandboxManager.isSandboxingEnabled()`、
`dangerouslyDisableSandbox` 标志、用户配置 `sandbox.excludedCommands` 三个维度。

epic #490 中曾把它列为 Phase 4.2 待移植项。本 ADR 正式判定：**不移植此模块**，并固化
背后的架构分歧，避免未来 review 反复追问。

## 2. 两种沙箱模型对比

### 2.1 上游模型：per-command opt-in 沙箱

```
agent process (host) ─┬─ bash command A → spawn into sandbox
                     ├─ bash command B → run on host (matched excludedCommands)
                     └─ bash command C → spawn into sandbox
```

- **agent 进程本身跑在主机上**，不进沙箱
- 每条 bash 命令通过 `shouldUseSandbox()` 决策后 spawn 子进程进沙箱（或不进）
- 用户可通过 `sandbox.excludedCommands`（如 `docker ps`、`bazel build`）让特定命令
  绕过沙箱以访问真实 Docker socket / 主机文件系统
- `dangerouslyDisableSandbox` 是 per-invocation override
- 决策粒度：**单条命令**

### 2.2 x-claw / ironclaw 模型：process-level 沙箱

证据：
- [desktop-client/ironclaw/src/orchestrator/job_manager.rs](../../desktop-client/ironclaw/src/orchestrator/job_manager.rs) 注释明确两种容器形态：
  「persistent containers with their own agent loops (as opposed to **ephemeral per-command containers**)」
- [docs/plans/architecture-refactor/p0a-sandbox-activation-inventory.md](./p0a-sandbox-activation-inventory.md)：
  `OsExecutor::new(timeout, allow_full_access)` 通过 macOS Seatbelt / Linux Landlock+seccomp 把
  **整个 agent 进程**置于沙箱
- ADR-002 / ADR-121 / ADR-119 描述的 Docker mode 同样是**整个 agent 容器**

```
[sandbox boundary]
┌────────────────────────────────────┐
│ agent process (in OS sandbox /     │
│   Docker container)                 │
│  ├─ bash command A                  │
│  ├─ bash command B  ← 自动继承沙箱  │
│  └─ bash command C                  │
└────────────────────────────────────┘
```

- **agent 进程整体跑在沙箱里**（OS sandbox 或 Docker container）
- 它派生的所有 bash 子进程**自动继承沙箱约束**，无 opt-in / opt-out 决策
- 决策粒度：**agent 启动时**（一次性）

## 3. 决策

**不移植 `shouldUseSandbox.ts`**。理由：

| 上游能力 | 在 x-claw 架构下的处理 |
|---|---|
| "这条命令该不该进沙箱"决策 | **不存在该决策点**：进程已在沙箱内，无逃逸路径 |
| `sandbox.excludedCommands`（让特定命令绕过沙箱） | **架构上不可行**：OS sandbox 无法 selectively unseal；Docker container 无法 inline 出来 |
| `dangerouslyDisableSandbox` per-invocation override | **架构上不可行**：同上 |
| `SandboxManager.isSandboxingEnabled()` 全局开关 | ✅ ironclaw 已有等价：`SandboxModeConfig.enabled` / `OsExecutor::allow_full_access` |

## 4. 后果与权衡

### 4.1 x-claw 模型的优势

- **安全默认**：默认无 per-command 逃逸路径，攻击面更小（OWASP A06: Vulnerable Components — 减少配置错误导致的暴露）
- **实现更简单**：不维护两个 executor（sandboxed + unsandboxed）
- **审计语义清晰**：所有 bash 命令在同一安全语境下，audit log 与威胁建模都简单

### 4.2 x-claw 模型的代价（已知放弃的能力）

- ❌ **无法支持** `docker ps`、`bazel build`、原生 GPU 访问等需要"打洞"到主机的命令
  - 上游通过 `excludedCommands` 解决这类需求
  - x-claw 当前只能要么**全开沙箱**（这些命令失败）要么**全关沙箱**（`allow_full_access=true`）
- ❌ 无法做 per-命令的"危险命令必入沙箱、安全命令直跑主机"的成本优化
- ❌ 与上游用户配置不互通：上游用户的 `sandbox.excludedCommands` 在 x-claw 无对应配置面

### 4.3 未来扩展位（不在本 ADR 范围）

如果将来需要等价于 `excludedCommands` 的能力，**必须开独立架构 epic「双 executor 架构」**：

- 维护 `SandboxedExecutor`（当前 OsExecutor / Docker）+ `UnsandboxedExecutor`（直跑主机）
- 在 `x_claw_agent` 的 bash 工具层做 per-command 决策（届时才需要 port `shouldUseSandbox.ts` 的逻辑）
- 需要重新做威胁建模（unsandboxed 通道是新的攻击面）
- 需要 admin-backend 配置面 + DLP 审计扩展

这不是 epic #490（bash-validation parity）的范围，本 ADR **不预设**会做。

## 5. 影响范围

- ✅ epic #490 Phase 4.2 标记为 **won't-fix — architecture divergence**
- ✅ `claude-code-main/src/tools/BashTool/shouldUseSandbox.ts` 从 parity 待办列表移除
- ✅ 不影响已就绪的 `BashValidationHook` / `BashPermissionHook` / `CompositeSafetyHook`
- ✅ 不改变 Phase 2.3（#608 Desktop Ask UX）与 Phase 4.2 sed hook（#607）的范围

## 6. 验证

- [x] 上游 `shouldUseSandbox.ts` 153 LOC 完整阅读 + 三个决策维度梳理
- [x] x-claw/ironclaw 沙箱实现 3 层验证：
  - semantic 等价：`grep_search "per_command|excludedCommands|shouldUseSandbox"` → 仅命中
    `job_manager.rs:4` 注释 + `linux_run_main_tests.rs:110`
  - 引用点等价：`job_manager.rs:4` 明确"ephemeral per-command containers"作为既有架构形态
  - 文档等价：ADR-002 / ADR-119 / ADR-121 / p0a-sandbox-activation-inventory.md 均描述
    process-level 沙箱模型
- [x] epic #490 closeout comment 已贴
- [x] Phase 4.2 won't-fix 决策已在 epic #490 公开记录

## 7. 撤销条件

本 ADR 在以下情况应被 revisit：

1. 用户需求出现"必须支持 `docker ps`、`bazel build` 等打洞命令"的硬约束
2. ironclaw 沙箱模型升级，支持 selective unseal（架构上变得可行）
3. 上游 claude-code 沙箱模型重构，与 x-claw 趋同（届时再决定要不要再对齐）

## 8. Refs

- 上游：`claude-code-main/src/tools/BashTool/shouldUseSandbox.ts`
- 上游：`claude-code-main/src/tools/BashTool/bashPermissions.ts` §`stripSafeWrappers` / `splitCommand_DEPRECATED`
- ADR-002 / ADR-119 / ADR-121 / 41-docker-vs-os-sandbox-capability-comparison.md
- p0a-sandbox-activation-inventory.md
- epic #490, PR #606, issue #607, issue #608
