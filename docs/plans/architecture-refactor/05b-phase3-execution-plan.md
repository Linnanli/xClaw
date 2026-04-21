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
