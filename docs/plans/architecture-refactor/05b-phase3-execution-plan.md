# Phase 3 — 修订版执行计划（基于能力审计 + LoopDelegate 发现）

> **状态**：Step A、B、C 已完成并提交。本文档把 Step D 之后重写为"搬 ironclaw 自己的代码、不从上游搬"的版本。
> **上游基线**：`claw-code@610b3470`，详见 [`crates/x_claw_agent/UPSTREAM_BASELINE.md`](../../../crates/x_claw_agent/UPSTREAM_BASELINE.md)
> **原始计划**：[`05-phase3-agent-extraction.md`](./05-phase3-agent-extraction.md)

---

## 关键发现（2026-04-21）

1. **ironclaw `agent/` 与上游 `claw-code/rust/crates/runtime/src/` 文件名几乎不相交**。上游只有 3 个模块值得跟进（`bash_validation`、`permissions+permission_enforcer`、`recovery_recipes`），且全部降级为 follow-up，不进 Phase 3。
2. **`agent/agentic_loop.rs` 已经提供了干净的 `LoopDelegate` trait**（chat dispatcher / job worker / container runtime 三种消费者都 `impl` 它）。这就是我们要的 agent runtime seam——不用从 claw-code 搬 conversation loop，直接把这块挪进 `x_claw_agent` 并在里面插 `SafetyHook`/`ApprovalGate` 调用点即可。
3. **ironclaw `agent/*` 有两类文件**：一类是 **agent runtime 本身**（LLM 循环、session、dispatcher），另一类是 **ironclaw 原创扩展**（routine/scheduler/self_repair/cost_guard/heartbeat）。切分的直觉边界已经清晰。

---

## 模块归属清单

### `crates/x_claw_agent`（agent runtime 核心）

| 文件 | 行数 | 归属理由 |
|---|---|---|
| `agent_loop.rs` | 1908 | 主循环入口，`Agent` / `AgentDeps` |
| `agentic_loop.rs` | 836 | `LoopDelegate` trait + `run_agentic_loop` |
| `dispatcher.rs` | 3001 | 工具调度（chat 消费者的 `LoopDelegate` 实现） |
| `thread_ops.rs` | 2572 | 线程持久化 + undo replay |
| `session.rs` | 1962 | Session/Thread/Turn 模型 |
| `session_manager.rs` | 1105 | 多用户 session 生命周期 |
| `submission.rs` | 870 | 提交解析（含 slash commands） |
| `commands.rs` | 1033 | 系统命令 handler |
| `compaction.rs` | 899 | 上下文压缩 |
| `context_monitor.rs` | 236 | 上下文预算监控 |
| `attachments.rs` | 307 | 附件内容增强 |
| `task.rs` | 283 | scheduler 用的 Task 类型 |
| `undo.rs` | 376 | 检查点 undo |
| `router.rs` | 200 | 消息意图路由 |
| **合计** | ~15 588 行 | |

### `crates/ironclaw_routines`（ironclaw 原创扩展）

| 文件 | 行数 | 归属理由 |
|---|---|---|
| `routine.rs` | 1562 | 类型定义 |
| `routine_engine.rs` | 2578 | 执行引擎 |
| `scheduler.rs` | 1230 | 调度器 |
| `self_repair.rs` | 856 | 卡住 job 自愈 |
| `cost_guard.rs` | 892 | 单用户成本上限 |
| `heartbeat.rs` | 971 | 主动心跳 |
| `job_monitor.rs` | 534 | 后台 job 输出转发 |
| **合计** | ~8 623 行 | |

### 保留在 `desktop-client/ironclaw/src/` 顶层（应用层组装）

- `src/agent_app.rs`（新，极简）：组装 Agent + Hook 实现
- `src/agent/mod.rs`（删除）：用新的 `agent_app` 替代
- 所有 channels / tools / hooks 注册：保持在应用层

---

## Step D 修订版：搬运 + 插 hook（~3 天）

### 原计划的问题

原 Step D 假设"从 claw-code `runtime/` 复制 `agent_loop.rs` 等"——这些文件**不在上游**，所以原文其实是"从 ironclaw 搬"，只是文档没写明。

### 修订后的顺序

**D-1 (~0.5d)**：把 **agent runtime** 里依赖最少的几个文件先搬：
- `session.rs`、`submission.rs`、`task.rs`、`attachments.rs`、`context_monitor.rs`、`undo.rs`

这些是数据类型和小工具，依赖 `crate::llm::*`、`ironclaw_common::*`、`crate::workspace::*`。需要：
- 把对 `crate::llm` 的依赖收窄成只用 `claw-code-api` 的类型（已经是 Phase 2 Step I 的成果）
- 把对 `ironclaw_common` 的引用保留（`ironclaw_common` 本就是共享 crate）
- 把对 `crate::workspace` 的依赖替换成泛型 `W: Workspace` 或 trait 对象（需要新 trait，放在 D-3）

**D-2 (~0.5d)**：搬 **compaction** 和 **router**：
- `compaction.rs`、`router.rs`

**D-3 (~1d)**：定义 `Workspace` / `Database` / `ChannelManager` / `ExtensionManager` 的 trait 占位，让 `x_claw_agent` 不直接依赖 ironclaw 应用层。

**D-4 (~0.5d)**：搬 `agentic_loop.rs`（`LoopDelegate` + `run_agentic_loop`），在里面插 4 个 hook 调用点：

| 位置 | Hook 调用 |
|---|---|
| `call_llm` 之前（delegate 组装完 reasoning/context 后） | `SafetyHook::before_prompt` |
| `handle_text_response` 之前（LLM 返回完整 text 后） | `SafetyHook::after_completion` |
| `execute_tool_calls` 循环内，每个 `ToolCall` 执行前 | `SafetyHook::before_tool_call` + 如 Block 则跳过，再 `ApprovalGate::request` |
| `execute_tool_calls` 循环内，tool 返回 output 后 | `SafetyHook::after_tool_output` |

Hook 以 `Arc<dyn SafetyHook>` / `Arc<dyn ApprovalGate>` 形式存于 `LoopDelegate` 实现里或通过新字段传入 `run_agentic_loop`。具体方案 Step D-4 开工时决定（两种都可行，看哪个 diff 最小）。

**D-5 (~0.5d)**：搬 `agent_loop.rs`、`dispatcher.rs`、`thread_ops.rs`、`commands.rs`、`session_manager.rs`。这是最大一搬，但文件内部自包含，主要是改 `use crate::...` 路径。

每次搬完跑一次 `cargo build -p x_claw_agent` + `cargo build -p desktop-client --lib`。

---

## Step E — `ironclaw_sandbox` crate（~1.5d）

与原计划相同：把 `desktop-client/ironclaw/src/sandbox/` 提取到 `crates/ironclaw_sandbox/`，并为 `SandboxManager` 实现 `x_claw_agent::SandboxExecutor`：

```rust
#[async_trait]
impl SandboxExecutor for SandboxManager {
    async fn run_bash(&self, req: SandboxExecRequest) -> Result<SandboxExecOutput, SandboxError> {
        let out = self.execute(&req.command, &req.cwd, req.env).await
            .map_err(|e| SandboxError::ExecutionFailed(e.to_string()))?;
        Ok(SandboxExecOutput {
            exit_code: out.exit_code,
            stdout: out.stdout,
            stderr: out.stderr,
            output: out.output,
            duration_ms: out.duration.as_millis() as u64,
            truncated: out.truncated,
        })
    }
    // read_file / write_file: 走 SandboxManager 的新 API 或 proxy 到 container
}
```

---

## Step F — `ironclaw_secrets` crate（~1d）

把 `desktop-client/ironclaw/src/secrets/` 提到 `crates/ironclaw_secrets/`，在 `SecretsStore` 之外加一个薄的 `AgentSecrets` 适配器实现 `x_claw_agent::SecretProvider`：

```rust
pub struct AgentSecrets<S: SecretsStore> {
    store: S,
    user_id: String,
}

#[async_trait]
impl<S: SecretsStore> SecretProvider for AgentSecrets<S> {
    async fn get(&self, key: &str) -> Result<Option<SecretString>, SecretError> {
        match self.store.get_decrypted(&self.user_id, key).await {
            Ok(s) => Ok(Some(SecretString::new(s.value))),
            Err(ironclaw::secrets::SecretError::NotFound(_)) => Ok(None),
            Err(e) => Err(SecretError::Io(e.to_string())),
        }
    }
    async fn list_names(&self) -> Result<Vec<String>, SecretError> {
        let refs = self.store.list(&self.user_id).await
            .map_err(|e| SecretError::Io(e.to_string()))?;
        Ok(refs.into_iter().map(|r| r.name).collect())
    }
}
```

---

## Step G — `ironclaw_safety` 实现 `SafetyHook`（~0.5d）

在 `crates/ironclaw_safety/` 下加 feature `agent-hook`：

```toml
[features]
agent-hook = ["dep:x_claw_agent"]
```

`src/agent_hook.rs`：把 `SafetyLayer::sanitize_tool_output` 映射到 `after_tool_output`，`LeakDetector::scan_and_clean` 映射到 `after_completion`，`Policy::check` 驱动 `before_tool_call` 的 Allow/Block 决策。

---

## Step H — `ironclaw_routines` crate（~1.5d）

按模块归属清单搬 7 个文件。这是独立的 crate，依赖 `x_claw_agent` + `ironclaw_sandbox`（后者通过 trait 访问）。

注意：`routine_engine.rs` 里的 `SandboxReadiness` 现在走 trait，而不是直接依赖 `SandboxManager`。

---

## Step I — 应用层组装（~1d）

`desktop-client/ironclaw/src/agent_app.rs`（新建，<100 行）：

```rust
use std::sync::Arc;
use x_claw_agent::{Agent, SafetyHook, SandboxExecutor, SecretProvider, ApprovalGate};
use ironclaw_safety::SafetyLayer;
use ironclaw_sandbox::SandboxManager;
use ironclaw_secrets::AgentSecrets;

pub fn build_agent(
    config: AppConfig,
    approval_tx: mpsc::Sender<ApprovalRequest>,
) -> Agent { /* ... */ }
```

---

## Step J — 删除旧 `agent/*`（~1d）

按原计划：在 A-I 全绿后删除整个 `desktop-client/ironclaw/src/agent/`。所有 `use crate::agent::...` 改成 `use x_claw_agent::...` 或 `use ironclaw_routines::...`。

---

## Step K — 语义 cherry-pick 演练（~1d）

原计划的"subtree pull 演练"降级为"语义 cherry-pick 演练"：把上游 `runtime/bash_validation.rs`（1004 行）作为一个 `x_claw_agent::safety::bash_validation` 新模块 port 过来，验证 porting log 机制可用。

演练目的：证明 `crates/x_claw_agent/UPSTREAM_BASELINE.md` 的 porting log 流程真能用；如果 port 很痛苦，说明 crate 边界或 hook 设计需要调整。

---

## 修订版估时

| Step | 原估 | 修订 | 理由 |
|---|---|---|---|
| A | 1d | ✅ 已完成 | baseline + 上游能力审计 |
| B | 0.5d | ✅ 已完成 | crate 骨架 |
| C | 1d | ✅ 已完成 | Hook traits + 7 单测 |
| D | 3d | 3d | 搬 ironclaw agent runtime + 插 4 个 hook 点 |
| E | 1.5d | 1.5d | sandbox crate + SandboxExecutor impl |
| F | 1d | 1d | secrets crate + SecretProvider impl |
| G | 0.5d | 0.5d | safety 实现 SafetyHook |
| H | 1.5d | 1.5d | routines crate |
| I | 1d | 1d | agent_app 组装 |
| J | 1d | 1d | 删 agent/* |
| K | 1d | 1d | 语义 cherry-pick 演练 |
| 缓冲 | 1.5d | 1.5d | |
| **合计** | 14d | **14d**（与原估持平，但 Step D 方向更明确） |

---

## 每步退出条件

| Step | 通过条件 |
|---|---|
| D | `cargo build -p x_claw_agent` 0/0；`cargo build -p desktop-client --lib` 0/0；原 agent/ 下仍保留（准备 Step J 再删） |
| E | `cargo test -p ironclaw_sandbox` 全绿；ironclaw `SandboxManager` 仍对内可用 |
| F | `cargo test -p ironclaw_secrets` 全绿 |
| G | `cargo test -p ironclaw_safety --features agent-hook` 全绿 |
| H | `cargo test -p ironclaw_routines` 全绿 |
| I | `cargo build -p desktop-client` 0/0；engine_startup_tests 全绿；真实 LLM 测试全绿 |
| J | `cargo build --all-features` 0/0；全 workspace `cargo test` 全绿 |
| K | 新 `bash_validation` 模块有单测；porting log 有新条目 |

---

## 已记入 porting log 的关键决策

- 2026-04-21：决定 agent runtime 从 ironclaw 搬，不从 claw-code 搬
- 2026-04-21：决定 `LoopDelegate` 作为 agent runtime 的主 seam，hook 调用从 `run_agentic_loop` 内部发起
- 2026-04-21：决定 Step K 从 "subtree pull 演练" 降级为 "语义 cherry-pick 演练"（候选：`bash_validation.rs`）

---

## 当前进度审查（最新一次盘点）

> 盘点方式：`ls crates/ + crates/x_claw_agent/src/`、对比 ironclaw `agent/*`、grep `agentic_loop.rs` 里的 `SafetyHook|ApprovalGate` 调用点。

### 已完成（Step A + B + C + 部分 D + D-3 + **D-4** + 部分 E/F/G）

| 产物 | 位置 | 备注 |
|---|---|---|
| 上游基线 + 能力审计 | `crates/x_claw_agent/UPSTREAM_BASELINE.md` | 含 Porting log |
| crate 骨架 | `crates/x_claw_agent/{Cargo.toml, src/lib.rs}` | 最小依赖集 |
| Hook traits + 默认实现 | `src/hooks.rs` | `SafetyHook / SandboxExecutor / SecretProvider / ApprovalGate` + `Noop / Auto / Deny / InMemory` 默认 |
| **`HookBundle` 聚合结构** (D-4) | `src/hooks.rs` | 四 `Arc` 字段 + `HookBundle::noop()` + 自定义 Debug |
| **`run_agentic_loop` Hook 插桩** (D-4) | `src/agentic_loop.rs` | `before_prompt` (Block→Failure) + `after_completion` (in-place)；tool 级 hook 由 delegate 调 |
| **D-4 契约测试 ×4** | `src/agentic_loop.rs::tests` | noop 透传 / Block 短路 / Redact in-place（LLM 看到 redacted）/ Err 透传 |
| `LlmCompleter` / `WorkspaceWriter` 占位 | `src/traits.rs` | Step D-3 主体已完成 |
| 应用层桥接 | `desktop-client/ironclaw/src/agent/traits_impl.rs` | ironclaw 侧实现 x_claw_agent 的 traits |
| 上游 port | `src/bash_validation.rs` (1004 行) + `src/permissions.rs` | Step K 预演产物 |
| 已搬运的 ironclaw agent runtime 支撑模块（**10 个**） | `src/{agentic_loop, compaction, context_monitor, intent, messages, reasoning_ctx, response_types, session, submission, task, undo}.rs` | 编译通过 + Hook 已插桩 |
| **G `IronclawSafetyHook` adapter** | `desktop-client/ironclaw/crates/ironclaw_safety/src/agent_hook.rs` | `SafetyLayer` → `SafetyHook` 映射，`agent-hook` feature 已开启 |
| **G 3 处生产接线** | `worker/container.rs` / `worker/job.rs` / `agent/dispatcher.rs` | 通过 `hook_bundle_with_safety()` helper 统一构造 |
| **E `SandboxAgentExecutor` adapter** | `desktop-client/ironclaw/src/sandbox/agent_executor.rs` | `SandboxManager` → `SandboxExecutor`；⚠️ **基于错误假设建立，见下文 "E 章节重新定义"** |
| **F `AgentSecrets` adapter** | `desktop-client/ironclaw/src/secrets/agent_provider.rs` | `SecretsStore` → `SecretProvider`；**尚未接线** |
| Integration contract test | 子模块 commit `79ec4f05` | |

### 待开发清单（剩余 D-5 + E/F 接线 + H/I/J/K）

#### ⚡ D-4 · Hook 调用点插桩（**关键阻塞项**）

在 `crates/x_claw_agent/src/agentic_loop.rs` 的 `run_agentic_loop` 内部插 4 处 Hook 调用：

| 位置 | Hook 调用 | 失败处理 |
|---|---|---|
| `call_llm` 之前 | `SafetyHook::before_prompt(&mut prompt)` | `Block` → 返回 `HostError::SafetyBlocked` |
| LLM 返回 text 后、交给 delegate 前 | `SafetyHook::after_completion(&mut completion)` | `Err` → `HostError::SafetyError` |
| `execute_tool_calls` 循环内，每个 `ToolCall` 前 | `SafetyHook::before_tool_call(&tool, &args)` → `ApprovalGate::request(...)` | `Block` 或 `Deny` → skip + 记录 deny reason |
| 工具 output 回传给 LLM 前 | `SafetyHook::after_tool_output(&tool, &mut output)` | `Err` → `HostError::SafetyError` |

**✅ 已完成**（parent `cf9eca75` / submodule `19d215e0`）：选择 `HookBundle` 入参方案（diff 最小，未污染 `LoopDelegate` 三实现方）。4 个契约测试全绿，`cargo test -p x_claw_agent` 202 passed。Tool 级 hook（`before_tool_call / after_tool_output / Approval`）仍由 `LoopDelegate::execute_tool_calls` 各 impl 自行调用，不在 loop 主干。

#### D-5 · 7 个大文件搬迁到 `x_claw_agent`（~1.5d）

| 文件 | 行数 | 依赖改造 |
|---|---|---|
| `agent_loop.rs` | 1908 | `AgentDeps` 字段中的 `workspace / db / channels / extensions` 需走新 trait 占位 |
| `dispatcher.rs` | 3001 | 工具调用前插 `ApprovalGate`；参数脱敏继续走 `SafetyHook` |
| `thread_ops.rs` | 2572 | 文件系统访问统一走 `SandboxExecutor` |
| `session_manager.rs` | 1105 | 多用户会话生命周期 |
| `commands.rs` | 1033 | 系统命令 handler |
| `attachments.rs` | 307 | 附件增强逻辑 |
| `router.rs` | 200 | 消息意图路由 |

**顺序建议**：`attachments → router → session_manager → commands → thread_ops → dispatcher → agent_loop`（依赖从小到大）。

**验收**：每搬一个就跑 `cargo build -p x_claw_agent` 与 `cargo build -p desktop-client --lib`。

#### E · 与上游 engine v2 进程外沙箱对齐（**章节重新定义**）

> **2026-04-23 修订**：原 E 计划（"提 `crates/ironclaw_sandbox` crate + 给 `x_claw_agent::SandboxExecutor` hook 接线 `SandboxManager`"）基于错误假设。下文为按上游 ironclaw main 分支（HEAD `9dcd8969`，代码在 `ironclaw-main/` 本地参考）真实架构重新规划。

##### E-现状诊断（必读）

- `SandboxManager` 在 **agent 主进程 0 个调用者**（上游 main + 本仓一致，git grep 已验证）
- `SandboxModeConfig::to_sandbox_config()` 在上游 main + 本仓都 **0 个调用者**
- `config.sandbox.enabled` 的 6 处 gate 全部控制 `container_job_manager` / `prompt_queue` / `SandboxReaper` / `SandboxReadiness` 枚举，**不控制** `SandboxManager`
- 唯一真实的 `ShellTool::with_sandbox(...)` 调用点是 `src/bin/sandbox_daemon.rs` —— 独立 binary，在容器**内部**跑
- 结论：**agent 主进程不走进程内 sandbox hook**，上游设计就是进程外 daemon + Docker exec NDJSON RPC

##### E-上游真实架构（`ironclaw-main/` 参考）

```
agent 主进程（宿主机）
    │
    │  LLM 决定调用 shell / file_read / ...
    ▼
ironclaw_engine::WorkspaceMounts  ← 抽象，按前缀分发
    │
    │  路径以 /project/ 开头？
    ▼
bridge/sandbox/intercept::maybe_intercept
    │
    │  SANDBOX_ENABLED=true ?
    ▼
ContainerizedMountFactory → ProjectSandboxManager
    │  （每项目一个 Docker 容器）
    ▼
DockerTransport::dispatch  ← NDJSON over "docker exec -i"
    │
    ▼
容器内 sandbox_daemon binary（255 行）
    │
    │  解析 Request { method: "execute_tool", params: { name, input } }
    ▼
ShellTool / FileReadTool / ...（在容器里执行，返回 Response）
```

关键组成（上游路径 → 本仓缺失情况）：

| 上游文件/目录 | 行数 | 本仓状态 |
|---|---|---|
| `crates/ironclaw_engine/` | 大（含 `WorkspaceMounts / MountBackend / ProjectMountFactory` 等） | ❌ 完全缺失 |
| `src/bridge/sandbox/` | 2624（11 个 `.rs`） | ❌ 完全缺失（本仓无 `src/bridge/` 目录） |
| `src/bin/sandbox_daemon.rs` | 255 | ❌ 完全缺失 |
| `SANDBOX_ENABLED` → `engine_v2_sandbox_enabled()` 开关 | 小 | ❌ 本仓只有 `config.sandbox.enabled`，未 gate 真实 sandbox |
| `src/sandbox/manager.rs::SandboxManager` (engine v1) | 本仓有 | ⚠️ engine v1 残留，上游也保留但不再是主路径 |

本仓 fork 时间点早于 engine v2 sandbox 落地：
- 分叉 commit: `732c23e06`
- 领先上游：38 commit（Phase 2/3 改造）
- **落后上游：387 commit**（engine v2 + bridge + sandbox_daemon 都在这 387 里）

##### E-重新定义：本轮 Phase 3 只做文档冻结，真实对齐进 Phase 4

| 子步 | Phase | 内容 | 可启动 |
|---|---|---|---|
| **E-0** | Phase 3 本轮 | 05b 文档冻结 + `SandboxAgentExecutor` / `SandboxExecutor` hook 加注释说明"Phase 3 不接线，保留作可选契约" | ✅ 立即 |
| **E-1** | Phase 3 本轮 | ADR：为何 Phase 3 不走进程内 sandbox hook，Phase 4 对齐上游 | ✅ 立即 |
| **E-2** | Phase 4 | 引入 `ironclaw_engine` crate（或 subtree pull 上游对应路径） | ❌ 依赖 Phase 4 启动 |
| **E-3** | Phase 4 | 搬运 `src/bridge/sandbox/` 11 个文件（~2624 行）到本仓；切出 `src/bridge/` 目录树 | ❌ 依赖 E-2 |
| **E-4** | Phase 4 | 新增 `[[bin]] sandbox_daemon` target（搬 `src/bin/sandbox_daemon.rs`） | ❌ 依赖 E-3 |
| **E-5** | Phase 4 | Dockerfile 改造：打包 `sandbox_daemon` 二进制进 worker 镜像 | ❌ 依赖 E-4 |
| **E-6** | Phase 4 | 端到端集成测试：宿主机 + 容器 + NDJSON RPC round-trip；失败路径（Docker 未启动/容器崩溃/daemon 不响应） | ❌ 依赖 E-5 |
| **E-7** | Phase 4 | 清理本仓 engine v1 `src/sandbox/manager.rs::SandboxManager` 的死代码残留（或保留为 legacy fallback，视上游策略） | ❌ 依赖 E-6 |

##### `x_claw_agent::SandboxExecutor` hook 保留策略

进程内 `SandboxExecutor` trait 已在 `crates/x_claw_agent/src/hooks.rs` 就位。处置策略：

- **保留不删**：作为 `x_claw_agent` 可选契约，方便未来 wasm sandbox / 其他 runtime 场景
- **不在 Phase 3 接线**：本仓 ironclaw 侧永远是 `NoopSandboxExecutor`
- **不把 `SandboxAgentExecutor` adapter 删掉**：它是基于错误假设建的，但代码本身无害、编译通过、有测试覆盖；留作文档反例 + 潜在未来复用（如果真的要把 engine v1 `SandboxManager` 挂进 hook，代码在那里）。文件头部加注释说明"Phase 3 未接线原因"。

##### 为什么不在 Phase 3 合并 E-2~E-7

1. **工作量**：上游 387 commit 差距，engine v2 + bridge 落地跨 agent loop / workspace / channels 整条链，绝不是"搬 2624 行"那么简单
2. **边界清晰**：Phase 3 的目标是"抽出 `x_claw_agent`"，不是"对齐上游 engine v2"
3. **风险控制**：engine v2 sandbox 涉及 Docker、NDJSON 协议、容器生命周期，调试需要真实环境，打包进 Phase 3 会阻塞整体进度
4. **上游持续演进**：Phase 4 开始时建议先同步上游最新 main，再开工 E-2

#### F · `crates/ironclaw_secrets` 新 crate（~1d）

**现状**：`AgentSecrets<S>` adapter 已在 `desktop-client/ironclaw/src/secrets/agent_provider.rs` 就位，**但尚未接线**。同 E，先接线后提 crate。

#### G · `ironclaw_safety` 加 `agent-hook` feature

**✅ 已完成**（parent `22236f99` / submodule `1dbcf861`）：

- `agent-hook` feature 已在 `desktop-client/ironclaw/Cargo.toml` 开启
- `IronclawSafetyHook` adapter 已实现 `before_prompt / after_completion / before_tool_call / after_tool_output`
- `hook_bundle_with_safety(Arc<SafetyLayer>)` helper 已在 `desktop-client/ironclaw/src/agent/agentic_loop.rs` 就位
- 3 处生产 call site（`worker/container.rs` / `worker/job.rs` / `agent/dispatcher.rs`）已通过该 helper 注入真实 `SafetyHook`
- `cargo build --workspace --tests` 0 错误 0 警告

**遗留**：契约测试（DLP 规则在 `SafetyHook` 接口下仍生效）尚未补充，建议在 E/F 接线时一并补上（参考 `DLP_TESTING_LESSONS_LEARNED.md` 的失败路径测试模式）。

#### H · `crates/ironclaw_routines` 新 crate（~1.5d）

未建立。按归属清单搬 7 个文件：

- `routine.rs` (1562) + `routine_engine.rs` (2578) + `scheduler.rs` (1230)
- `self_repair.rs` (856) + `cost_guard.rs` (892) + `heartbeat.rs` (971) + `job_monitor.rs` (534)

依赖 `x_claw_agent` + `ironclaw_sandbox`（走 trait）。

#### I · `desktop-client/ironclaw/src/agent_app.rs` 应用层组装（~1d）

新建 <100 行文件：组装 `Agent + SafetyLayer (impl SafetyHook) + SandboxManager (impl SandboxExecutor) + AgentSecrets + ApprovalDispatcher`。

#### J · 删除旧 `agent/*`（~1d）

`desktop-client/ironclaw/src/agent/` 全量删除。`use crate::agent::...` 全局替换为 `use x_claw_agent::...` 或 `use ironclaw_routines::...`。

**前置**：Step A–I 全绿 + `integration_smoke_tests.rs` 通过。

#### K · 语义 cherry-pick 演练（~1d）

挑一个上游新增/改动的模块（候选：`bash_validation.rs` 的增量、`recovery_recipes.rs`）走完整 porting 流程，在 `UPSTREAM_BASELINE.md` Porting log 加条目。

### 进度百分比（估算）

| 大块 | 进度 |
|---|---|
| Step A + B + C | 100% |
| Step D-1 ~ D-3 + D-4 + D-5 支撑模块 | ~85%（缺 7 个大文件搬迁） |
| Step D-4 Hook 插桩 | **100%** ✅ |
| Step G `ironclaw_safety` agent-hook | **100%**（接线 + 契约测试完成） |
| Step E（原）→ E-0/E-1 文档冻结 | **0%**（本轮立即） |
| Step E-2 ~ E-7（Phase 4 对齐上游 engine v2） | 0% |
| Step F adapter → F 接线 | adapter 已建，**未接线** |
| Step D-5 / H / I / J / K | 0% |
| **整体 Phase 3** | **~42%** |

### 建议切入顺序（2026-04-23 修订）

1. ~~**D-4（Hook 插桩）**~~ ✅ 已完成（`cf9eca75`）
2. ~~**G（`ironclaw_safety::agent_hook`）**~~ ✅ 已完成（`22236f99` + 契约测试 `a18aef97`）
3. **E-0 + E-1（本轮）** — 05b 文档冻结 + 写 ADR 记录"Phase 3 不走进程内 sandbox hook，Phase 4 对齐上游 engine v2"；给 `SandboxAgentExecutor` 加注释
4. **F 接线** — `AgentSecrets` 注入 `HookBundle.secrets`（通过扩展 helper `hook_bundle_with_safety_and_secrets`），3 处 call site 更新
5. **D-5（7 个大文件搬迁）** — 沿原顺序：`attachments → router → session_manager → commands → thread_ops → dispatcher → agent_loop`
6. **H（`ironclaw_routines`）** — 依赖 D-5
7. **I → J → K** — 应用层组装 + 清理 + 上游演练
8. **（Phase 4）E-2 ~ E-7** — engine v2 + `src/bridge/sandbox/` + `sandbox_daemon` binary 对齐上游
