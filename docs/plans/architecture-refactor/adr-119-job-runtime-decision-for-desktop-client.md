# ADR-119 — 桌面客户端 Job 运行时决策（Docker-less 默认 + Fail-Closed）

> **Status**: Accepted (W6 milestone)
> **Date**: 2026-05-12
> **Authors**: x-claw 架构组
> **Closes**: [#90 P1/W6 Job runtime decision for Docker-less desktop clients](https://github.com/Linnanli/xClaw/issues/90)
> **Builds on**: [42-client-job-runtime-routes.md](42-client-job-runtime-routes.md)（候选方案矩阵）、[41-docker-vs-os-sandbox-capability-comparison.md](41-docker-vs-os-sandbox-capability-comparison.md)
> **Related**: [48-agent-framework-readiness-and-task-audit.md §3.2 / §6.3](48-agent-framework-readiness-and-task-audit.md)

---

## 1. 背景

`#90` 在第一轮架构审计中提出：

> 当前架构保留 ironclaw job 工具，但桌面客户端目标可能是 Docker-less 或部分离线。审计指出 job 运行时策略需要明确的产品/架构决策，而不是事故性行为。

42 文档已经穷举了候选方案（本地多种容器运行时、云端、不实现），但留下的状态是 **「📋 决策搁置 · 仅记录候选方案，不立即执行」**。本 ADR 把它收口。

### 1.1 当前实现状态（事实）

实测当前 main（xClaw HEAD）的代码行为，不是空想：

| 维度 | 路径 | 当前实现 |
|---|---|---|
| **本地无沙箱（Scheduler）** | [ironclaw/src/tools/builtin/job.rs `execute_local`](../../../desktop-client/ironclaw/src/tools/builtin/job.rs) → `Scheduler::dispatch_job()` | 进程内 Worker，**无任何隔离**，享有 host 全部权限（FS/network/env），仅靠 `WriteFileTool` / `ShellTool` 自己的 OsExecutor 沙箱限制 |
| **Docker 沙箱** | [ironclaw/src/tools/builtin/job.rs `execute_sandbox`](../../../desktop-client/ironclaw/src/tools/builtin/job.rs) → `ContainerJobManager::create_job()` | bollard + Docker/OrbStack/Colima/Rancher/Podman 任一 socket，资源限额 + 只读 bind mount |
| **Docker 检测** | [ironclaw/src/sandbox/detect.rs `check_docker`](../../../desktop-client/ironclaw/src/sandbox/detect.rs) | 三态：`Available` / `NotInstalled` / `NotRunning`，平台特定安装提示 |
| **配置开关** | `config.sandbox.enabled` | `bool`，无第三态 |
| **当前 `enabled=true` + 无 Docker 时的行为** | [ironclaw/src/orchestrator/mod.rs L82-103](../../../desktop-client/ironclaw/src/orchestrator/mod.rs#L82) | `tracing::warn!`，把 `job_manager` 置 `None`，**自动降级**到本地 Scheduler 路径 |
| **Routine 路径** | [ironclaw/src/routines/routine_engine.rs L1273-1284](../../../desktop-client/ironclaw/src/routines/routine_engine.rs#L1273) | **已经 Fail-Closed**：`SandboxReadiness::DockerUnavailable` → `RoutineError::JobDispatchFailed` |
| **云端 Job** | — | **未实现** |

### 1.2 三个真实问题

1. **静默降级是政企默认下的安全漂移**
   `sandbox.enabled=true` + 无 Docker 时，当前 orchestrator 不会让 `create_job` 调用失败，而是悄悄走本地 Scheduler 路径。结果：用户以为自己在容器里跑 job，实际跑在 host 进程内。Routine 路径已经 fail-closed，但 chat-driven `create_job` 没对齐。

2. **政企默认值缺位**
   `config.sandbox.enabled` 默认 `false`（[ironclaw/src/config.rs](../../../desktop-client/ironclaw/src/config.rs)），意味着开箱即用 = 本地 Scheduler 全 host 权限。对政企桌面客户端这是不可接受的默认值。

3. **可见性三重门未对齐**
   42 文档讨论的是「可达 vs 不可达」，但没说「不可达时 LLM 是否还看见 `create_job` 工具」。当前 LLM 始终能看到 `create_job`，可见性策略没到位（P0-B triple-gate 仍是 follow-up）。

---

## 2. 决策

### 决策 D1：Job 运行时三档枚举（替换 `bool`）

把 `config.sandbox.enabled: bool` 重构成 `config.job_runtime: JobRuntimeMode`，三档：

```rust
pub enum JobRuntimeMode {
    /// 政企桌面默认。完全禁用 Job 工具：LLM 看不见、调不到。
    Disabled,
    /// 本地 Docker / OrbStack / Colima / Rancher / Podman 容器（已实现）。
    /// 启动时强制要求 Docker 可达，否则启动失败（fail-closed）。
    LocalContainer,
    /// 路由到云端 Job 服务（未实现，预留枚举位）。
    Cloud { endpoint: String },
}
```

**理由**：
- 三档枚举消除「`enabled=true` 但 Docker 不可用」的歧义状态
- 把"是否可见"和"用什么实现"两个正交问题同时表达
- `Cloud` 变体留位，未来不破坏 wire 兼容

### 决策 D2：政企桌面默认 = `Disabled`

**默认 `JobRuntimeMode::Disabled`**，即开箱即用零 Job 能力。

理由：
- 本地 Scheduler（进程内 Worker）**不是**安全隔离，让它做"默认值"会让用户以为有保护（实际无）
- 政企 IT 部署 Docker Desktop 受限/合规阻挡很常见，不能假设 Docker 可达
- "看不到 `create_job`" 比 "看到但调用失败" 用户心智更清晰

显式启用 = 明确写 `[job_runtime] mode = "local_container"` 或 `mode = "cloud"`。

### 决策 D3：Fail-Closed，不静默降级

`mode = "local_container"` 但 Docker 启动时不可达：

- **boot-time**：`Agent::new` 返回 `ConfigError::JobRuntimeUnavailable`（带 platform-specific install hint），客户端启动失败，**绝不**降级到本地 Scheduler
- **runtime**：Docker 中途失联（`ContainerJobManager::create_job` 报错），`create_job` 返回明确 `ToolError::ExecutionFailed`，**绝不**重试到本地 Scheduler
- **Routine 路径**：保持现状（已 fail-closed）
- **诊断**：`ironclaw doctor` 输出 `JobRuntimeMode` 与 `DockerStatus`，给用户一处查清的入口

### 决策 D4：可见性三重门绑定

`JobRuntimeMode::Disabled` → `tool_definitions` **不返回** `create_job` / `list_jobs` / `job_status` / `cancel_job` / `job_events` / `job_prompt`。

技术路径：复用 [tools/feature_flags.rs](../../../desktop-client/ironclaw/src/tools/feature_flags.rs) 的 `is_tool_enabled` 钩子，新增 `job_runtime_mode_disables(name) -> bool` 谓词。

**这绑定到 P0-B triple gate**（[#94](https://github.com/Linnanli/xClaw/issues/94)）：policy gate / approval gate / visibility gate 三层都要识别 `JobRuntimeMode::Disabled`，本 ADR 只交付 visibility 层。

### 决策 D5：审计与批准元数据

每次 `create_job` 成功执行（不论 mode），必须发出含以下字段的 `AppEvent::JobStarted`：

| 字段 | 必选 | 说明 |
|---|---|---|
| `job_id` | ✅ | UUID |
| `runtime_mode` | ✅ | `local_container` / `cloud`（`disabled` 不可能到这里） |
| `user_id` | ✅ | 触发用户 |
| `credential_grants` | ✅ | secret 名单 + env 名（值禁止落审计） |
| `project_dir` | ✅ | bind mount 绝对路径 |
| `image` / `endpoint` | ✅ | 容器镜像或云端 endpoint，便于审计追溯 |
| `parent_conversation_id` | 选 | 链回 chat session |

ironclaw `DataReporter` 透传到客户端审计后端。

### 决策 D6：云端 Job = 未来工作（P2）

`Cloud` 变体只是预留 Rust 枚举位，**本 ADR 不实现**：

- 客户端如何鉴权到云端 Job 服务（OIDC？mTLS？）
- 云端 Job 怎么访问用户本地文件（不访问？只跑 read-only 任务？）
- 离线行为（队列 + 网络恢复后 flush？还是直接报错？）
- 跨租户隔离

这些都是产品决策，不是架构决策，等有具体客户需求再开 ADR-N。

### 决策 D7：离线行为 = `Disabled` 时无影响 / `LocalContainer` 时仍然可用

- `JobRuntimeMode::Disabled`：本地 Scheduler 不存在，离线/在线无差异
- `JobRuntimeMode::LocalContainer`：Docker 引擎是本地服务，离线下完全可用（容器内 job 自己的网络访问由 sandbox network policy 决定，与 ironclaw 离线状态无关）
- `JobRuntimeMode::Cloud`：未来工作，离线策略由 D6 决定

---

## 3. 验收标准（issue #90 acceptance）

| #90 验收项 | 本 ADR 兑现 |
|---|---|
| Decision is recorded in an ADR or plan update | ✅ 本 ADR-119 |
| Enterprise default behavior is explicit | ✅ D2：默认 `Disabled` |
| Disabled/unsupported behavior is fail-closed and auditable | ✅ D3 + D5 |
| Follow-up implementation issues are created if the decision requires code changes | ✅ §5 列了 4 条 follow-up |

---

## 4. 非目标（What's NOT in this ADR）

- ❌ 不实现 `JobRuntimeMode::Cloud`（D6）
- ❌ 不重写 `ContainerJobManager` / `Scheduler::dispatch_job`（保留现有实现，只在外部包一层 mode 判定）
- ❌ 不引入新容器运行时候选（42 文档已穷举，本 ADR 只锁定"使用 Docker 兼容 API + bollard"这条已实现路径）
- ❌ 不动 P0-B triple gate 的 policy / approval 层（visibility 之外两层留给 #94）
- ❌ 不动 codex / claw-code（ADR-118 §2 readonly guard）

---

## 5. Follow-up issues

本 ADR 的实现拆 4 条独立 PR（各自不超过 effort:S）：

1. **F1**：把 `config.sandbox.enabled: bool` 迁移到 `config.job_runtime: JobRuntimeMode`，默认 `Disabled`，附 wire 向后兼容（`enabled=true` → `local_container` 一次性 migrate）
2. **F2**：`Agent::new` boot-time 校验 + `ironclaw doctor` 输出 `JobRuntimeMode`；删除 orchestrator 中 `enabled=true` 静默降级路径
3. **F3**：`tool_definitions_filtered` 引入 `job_runtime_mode_disables` 谓词，`Disabled` 模式下不暴露 6 个 job 工具
4. **F4**：`AppEvent::JobStarted` 加 `runtime_mode` / `credential_grants` / `image` / `endpoint` 字段，`DataReporter` 透传

每条 issue 单独开，单独标注 `wave:W6` + `area:jobs`，每条都依赖 F1（F1 落了之后 F2/F3/F4 可并行）。

---

## 6. 决策原因（为什么不是其他选项）

| 候选默认值 | 否决理由 |
|---|---|
| `LocalContainer` | 政企桌面 Docker 不可达常见，会导致开箱即用启动失败 |
| 「Local Scheduler with no isolation」 | 这是当前隐式默认，但等于"看起来有 Job 实则裸跑 host"，安全漂移 |
| 「检测 Docker，有就 LocalContainer，无就 Disabled」自动选择 | 让默认值依赖运行环境 = 不可预测 = 政企合规噩梦 |
| 直接走云端 Job | D6：未实现，且离线行为不明 |

**核心原则**：政企默认必须是**可预测的**、**fail-closed 的**、**显式 opt-in 强能力的**。
