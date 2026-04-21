# Phase 3 — Agent 能力抽离到 `x_claw_agent` crate

> **时长**：1-2 周
> **风险**：中（大量代码搬迁，但语义基本保持）
> **收益**：重新获得 claw-code 上游 rebase 能力；消除 agent 状态重复管理；`agent/*` 24041 行拆成清晰 crate 边界
> **前置**：Phase 1 + Phase 2 完成

---

## 目标

把 [`desktop-client/ironclaw/src/agent/`](../../desktop-client/ironclaw/src/agent) 拆成：

1. **`crates/x_claw_agent/`**（新）— 对齐 claw-code 的 runtime + commands，作为 x-claw 项目内的 fork，可定期 rebase 上游
2. **`crates/ironclaw_routines/`**（新）— ironclaw 原创的 routine / scheduler / self-repair 等扩展能力
3. **`desktop-client/ironclaw/src/agent_app.rs`**（极简）— 仅组装 crate + Hook 注入

同时把 `src/sandbox/` 和 `src/secrets/` 抽出为独立 crate，让 `x_claw_agent` 通过 trait 依赖它们。

---

## 非目标

- ❌ 不改变 agent 运行时行为（纯重构，不改 feature）
- ❌ 不升级 claw-code 上游版本（Phase 3 结束后再做首次 rebase）

---

## 代码盘点

[`desktop-client/ironclaw/src/agent/`](../../desktop-client/ironclaw/src/agent) 目录分类：

### 来自 claw-code（归入 `x_claw_agent`）

- `agent_loop.rs`（1824 行）
- `agentic_loop.rs`
- `dispatcher.rs`（2889 行）
- `thread_ops.rs`（2546 行）
- `session.rs`（1962 行）
- `session_manager.rs`
- `commands.rs`
- `compaction.rs`
- `context_monitor.rs`
- `submission.rs`
- `task.rs`

### ironclaw 原创（归入 `ironclaw_routines` 或保留在应用层）

- `routine.rs`
- `routine_engine.rs`
- `scheduler.rs`
- `self_repair.rs`
- `heartbeat.rs`
- `job_monitor.rs`
- `cost_guard.rs`
- `attachments.rs`
- `router.rs`（可能是应用路由逻辑，需核实）
- `undo.rs`

---

## 实施步骤

### Step A — 建立 claw-code 同步基线（~1 天）

选定 `claw-code/rust/crates/runtime` 某一 commit 作为 **Phase 3 起点基线**，记录在 `crates/x_claw_agent/UPSTREAM_BASELINE.md`：

```md
# Upstream Baseline
Source: claw-code/rust/crates/runtime @ <commit-sha>
Source: claw-code/rust/crates/commands @ <commit-sha>
Snapshot date: 2026-04-XX

## Porting log
- 2026-04-XX: Initial fork
```

同步策略：**git subtree + porting log**。后续上游更新时 `git subtree pull`，手动把 diff porting 过来。

---

### Step B — 创建 `crates/x_claw_agent` 骨架（~0.5 天）

```
crates/x_claw_agent/
  Cargo.toml
  src/
    lib.rs
    agent.rs           # 对外 Agent<L,S,X,K,A> 类型
    hooks.rs           # SafetyHook / SandboxExecutor / SecretProvider / ApprovalGate trait
    runtime/           # ← 从 claw-code runtime 复制
      agent_loop.rs
      dispatcher.rs
      session.rs
      thread_ops.rs
      compaction.rs
      ...
    commands/          # ← 从 claw-code commands 复制
    event.rs           # AgentEvent
    error.rs
  tests/
```

`Cargo.toml`：

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
serde = "1"
serde_json = "1"
async-trait = "0.1"
futures = "0.3"
claw-code-api = { path = "../../claw-code/rust/crates/api" }
claw-code-tools = { path = "../../claw-code/rust/crates/tools" }

[dev-dependencies]
tokio-test = "0.4"
```

注意：**不依赖任何 ironclaw 代码**，保持独立可复用。

---

### Step C — 定义 Hook trait（~1 天）

详细设计见 [`02-target-architecture.md`](./02-target-architecture.md#3-hook-trait-设计phase-3-关键)。此处列文件结构：

`crates/x_claw_agent/src/hooks.rs`：

```rust
// SafetyHook
#[async_trait]
pub trait SafetyHook: Send + Sync {
    async fn before_prompt(&self, prompt: &mut String) -> Result<SafetyDecision, SafetyError>;
    async fn after_completion(&self, completion: &mut String) -> Result<(), SafetyError>;
    async fn before_tool_call(&self, tool: &str, args: &Value) -> Result<SafetyDecision, SafetyError>;
}

// SandboxExecutor
#[async_trait]
pub trait SandboxExecutor: Send + Sync {
    async fn run_bash(&self, cmd: &str, cwd: &Path) -> Result<BashOutput, SandboxError>;
    async fn read_file(&self, path: &Path) -> Result<Vec<u8>, SandboxError>;
    async fn write_file(&self, path: &Path, data: &[u8]) -> Result<(), SandboxError>;
    async fn fetch(&self, req: HttpRequest) -> Result<HttpResponse, SandboxError>;
}

// SecretProvider
#[async_trait]
pub trait SecretProvider: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<SecretString>, SecretError>;
    async fn list_names(&self) -> Result<Vec<String>, SecretError>;
}

// ApprovalGate
#[async_trait]
pub trait ApprovalGate: Send + Sync {
    async fn request(&self, tool: &str, args: &Value) -> Result<ApprovalOutcome, ApprovalError>;
}
```

提供 `NoopSafetyHook`、`LocalSandboxExecutor`、`InMemorySecrets`、`AutoApproveGate` 作为默认实现，方便单测。

---

### Step D — 从 claw-code 搬代码并插入 hook 调用点（~3 天）

把 claw-code `runtime/` 代码复制到 `crates/x_claw_agent/src/runtime/`。

**最小侵入原则**：只在关键边界插入 hook 调用，尽量不改核心逻辑：

```rust
// agent_loop.rs（改动示例）
impl AgentLoop {
    async fn step(&mut self) -> Result<StepOutcome> {
        // 发 LLM 前
        if let Some(hook) = &self.hooks.safety {
            hook.before_prompt(&mut self.current_prompt).await?;
        }

        let delta_stream = self.llm.complete(req).await?;

        // 接收 LLM 输出后
        while let Some(delta) = delta_stream.next().await {
            if let CompletionDelta::TextDelta(t) = &delta {
                if let Some(hook) = &self.hooks.safety {
                    hook.after_completion(&mut t.clone()).await?;
                }
            }
            // ...
        }

        // 工具调用前
        if let ToolCallDecision::Run { tool, args } = decision {
            if let Some(hook) = &self.hooks.safety {
                match hook.before_tool_call(&tool, &args).await? {
                    SafetyDecision::Block(reason) => return Err(...),
                    _ => {}
                }
            }
            if let Some(gate) = &self.hooks.approval {
                match gate.request(&tool, &args).await? {
                    ApprovalOutcome::Denied => return Err(...),
                    ApprovalOutcome::ModifiedArgs(new) => args = new,
                    _ => {}
                }
            }
            let result = self.hooks.sandbox.run_tool(&tool, &args).await?;
            // ...
        }
    }
}
```

每个 hook 插入点都要 **porting log 记一笔**，方便未来从 claw-code rebase 时 resolve conflict。

---

### Step E — 抽出 `ironclaw_sandbox` crate（~1.5 天）

从 `desktop-client/src/sandbox/` 搬到 `crates/ironclaw_sandbox/`。

核心类型：
- `DockerSandbox`（现有）
- `HttpProxyGuard`（现有）
- 实现 `x_claw_agent::SandboxExecutor`

Cargo：
```toml
[dependencies]
x-claw-agent = { path = "../x_claw_agent" }  # 为了实现 trait
```

---

### Step F — 抽出 `ironclaw_secrets` crate（~1 天）

从 `desktop-client/src/secrets/` 搬到 `crates/ironclaw_secrets/`。实现 `SecretProvider`。

---

### Step G — `ironclaw_safety` 实现 SafetyHook（~0.5 天）

在 `crates/ironclaw_safety/` 内加一个可选 feature：

```toml
[features]
agent-hook = ["dep:x-claw-agent"]
```

`src/agent_hook.rs`：

```rust
#[cfg(feature = "agent-hook")]
use x_claw_agent::{SafetyHook, SafetyDecision, SafetyError};

#[cfg(feature = "agent-hook")]
#[async_trait]
impl SafetyHook for crate::Safety {
    async fn before_prompt(&self, prompt: &mut String) -> Result<SafetyDecision, SafetyError> {
        let result = self.scan(prompt);
        match result.severity {
            Severity::Block => Ok(SafetyDecision::Block(result.reason.clone())),
            Severity::Redact => { *prompt = result.redacted.clone(); Ok(SafetyDecision::Redact) }
            _ => Ok(SafetyDecision::Allow),
        }
    }
    // ...
}
```

---

### Step H — 抽出 `ironclaw_routines` crate（~1.5 天）

把 `routine.rs`、`routine_engine.rs`、`scheduler.rs`、`self_repair.rs`、`cost_guard.rs`、`heartbeat.rs` 等搬到 `crates/ironclaw_routines/`。

这些是 ironclaw 原创扩展，依赖 `x_claw_agent`：

```toml
[dependencies]
x-claw-agent = { path = "../x_claw_agent" }
ironclaw-sandbox = { path = "../ironclaw_sandbox" }
```

---

### Step I — 应用层组装（~1 天）

`desktop-client/ironclaw/src/agent_app.rs`（新，替代原大部分 `agent/*`）：

```rust
use x_claw_agent::Agent;
use ironclaw_safety::Safety;
use ironclaw_sandbox::DockerSandbox;
use ironclaw_secrets::KeychainSecrets;

pub fn build_agent(
    config: AppConfig,
    approval_tx: mpsc::Sender<ApprovalRequest>,
) -> Agent<
    ClawCodeLlmProvider,
    Safety,
    DockerSandbox,
    KeychainSecrets,
    TauriApprovalGate,
> {
    let llm = ClawCodeLlmProvider::new(config.llm);
    let safety = Safety::new(config.safety);
    let sandbox = DockerSandbox::new(config.sandbox);
    let secrets = KeychainSecrets::new();
    let approval = TauriApprovalGate::new(approval_tx);

    Agent::new(llm, safety, sandbox, secrets, approval)
}
```

整个文件 <100 行。

---

### Step J — 删除旧 `agent/*`（~1 天）

条件：
- [ ] Step A-I 全绿
- [ ] 集成测试覆盖（冒烟 + 真实 LLM）全绿

操作：

```bash
rm -rf desktop-client/ironclaw/src/agent/
# 修复所有因此产生的 import 错误
cargo build --all-features
cargo test --all
```

**预期删除行数**：~24000 行（搬到 crates/ 下，净行数略减，但耦合消失）。

---

### Step K — 首次 claw-code rebase 演练（~1 天）

等 Phase 3 稳定 1 周后，做一次"假装上游更新"的 rebase 演练：

1. 挑选 claw-code 上游最新 commit
2. `git subtree pull --prefix=claw-code ...`
3. 观察 `claw-code/rust/crates/runtime` 变化
4. 手动把对应变化 porting 到 `crates/x_claw_agent/src/runtime/`
5. 解决 hook 插入点附近的 conflict
6. 记录 porting log

演练目的：**证明架构真的支持 rebase**。如果演练痛苦，就是 hook 插入点设计有问题，要调整。

---

## 验收标准

- [ ] `crates/x_claw_agent/` 存在且可独立 `cargo test -p x-claw-agent`
- [ ] `crates/ironclaw_sandbox/`、`crates/ironclaw_secrets/`、`crates/ironclaw_routines/` 存在
- [ ] `desktop-client/ironclaw/src/agent/` 已删除
- [ ] 5 块安全能力全部通过 Hook 接入（见 [`06-safety-preservation.md`](./06-safety-preservation.md)）
- [ ] 端到端集成测试全绿
- [ ] claw-code rebase 演练完成，porting 工作量 <1 天
- [ ] `cargo build --all-features` 0 错 0 警

---

## 回滚策略

Phase 3 是纯重构，功能不变。回滚方案：

1. **Git 分支**：整个 Phase 3 在 `refactor/phase3-agent-extraction` 分支上做
2. **原子合并**：Step A-J 在单个 PR 中，通过 CI 后合并；出问题直接 revert PR
3. **不做 feature flag**：因为这是代码搬迁，双路径维护代价过高

风险缓释：Step D 每插一个 hook 都单独 commit，方便 bisect。

---

## 估时

| Step | 任务 | 估时 |
|------|------|------|
| A | 建立 upstream baseline | 1d |
| B | x_claw_agent crate 骨架 | 0.5d |
| C | Hook trait 定义 | 1d |
| D | 搬 claw-code 代码 + 插 hook | 3d |
| E | ironclaw_sandbox crate | 1.5d |
| F | ironclaw_secrets crate | 1d |
| G | safety 实现 SafetyHook | 0.5d |
| H | ironclaw_routines crate | 1.5d |
| I | 应用层组装 | 1d |
| J | 删除旧 agent/* | 1d |
| K | rebase 演练 | 1d |
| — | 缓冲 | 1.5d |
| **合计** | | **14d（两周）** |
