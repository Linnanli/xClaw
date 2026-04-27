# 02 — 目标架构

> 本文描述 Phase 3 全部完成后的终态。Phase 1/2 是通往此终态的中间步骤。

---

## 1. 终态总览

```
┌──────────────────────────────────────────────────────────────────┐
│ Frontend (desktop-client/src-ui)                                 │
│                                                                   │
│   <AssistantRuntimeProvider runtime={useChatRuntime(transport)}> │
│     └─ assistant-ui 原生组件（Thread/Messages/Composer/ToolUI）  │
│                                                                   │
│   工具 UI 通过 makeAssistantToolUI 注册：                        │
│     - approval-request-tool-ui.tsx                               │
│     - web-search-tool-ui.tsx                                     │
│     - write-file-tool-ui.tsx                                     │
│                                                                   │
│   Transport: class TauriTransport implements ChatTransport       │
│     └─ 把 invoke("chat_send") 的 SSE 流转换为 AI SDK DataStream  │
└──────────────────────────────────────────────────────────────────┘
                              │
                              │  Vercel AI SDK Data Stream Protocol
                              │  (0:text / 9:tool-call / a:tool-result / 2:data / d:finish)
                              ▼
┌──────────────────────────────────────────────────────────────────┐
│ Tauri Channel Layer (desktop-client/src/tauri_channel.rs)        │
│                                                                   │
│   - 监听 x_claw_agent 产出的 AgentEvent                          │
│   - 编码成 DataStreamFrame JSON                                  │
│   - 通过 channel::Channel / emit 发回前端                        │
│                                                                   │
│   ~300 行（从当前 678 行缩减）                                   │
└──────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌──────────────────────────────────────────────────────────────────┐
│ crates/x_claw_agent（新，fork 自 claw-code runtime + commands）  │
│                                                                   │
│   pub struct Agent<S: SafetyHook, X: SandboxExecutor, ...>       │
│   impl Agent {                                                    │
│       pub async fn run_turn(&self, input) -> AgentEventStream;   │
│   }                                                               │
│                                                                   │
│   内嵌 claw-code 的：                                            │
│     - agent_loop.rs        - session.rs                          │
│     - dispatcher.rs        - thread_ops.rs                       │
│     - compaction.rs        - context_monitor.rs                  │
│     - plan_mode / subagent / slash commands                      │
│                                                                   │
│   Hook trait 注入点：                                            │
│     trait SafetyHook    → 调用 ironclaw_safety                   │
│     trait SandboxExecutor → 调用 ironclaw_sandbox                │
│     trait SecretProvider → 调用 ironclaw_secrets                 │
│     trait ApprovalGate  → 由 tauri_channel 实现                  │
└──────────────────────────────────────────────────────────────────┘
       │               │              │                │
       ▼               ▼              ▼                ▼
┌──────────────┐ ┌──────────────┐ ┌─────────────┐ ┌────────────────┐
│ ironclaw_    │ │ ironclaw_    │ │ ironclaw_   │ │ claw-code/     │
│ safety       │ │ sandbox      │ │ secrets     │ │ rust/crates/   │
│              │ │              │ │             │ │ api            │
│ DLP /        │ │ Docker       │ │ Keychain +  │ │                │
│ Prompt inj   │ │ HTTP proxy   │ │ AES-GCM     │ │ Anthropic +    │
│ 防御         │ │ allowlist    │ │             │ │ OpenAI-compat  │
│              │ │              │ │             │ │ (OpenAI/xAI/   │
│ (已独立)     │ │ (新抽出)     │ │ (新抽出)    │ │  DashScope/    │
│              │ │              │ │             │ │  Kimi/Ollama)  │
└──────────────┘ └──────────────┘ └─────────────┘ └────────────────┘
```

---

## 2. Crate 布局

```
crates/
  ironclaw_auth/           # 已有 ✓
  ironclaw_safety/         # 已有 ✓ - DLP、prompt injection 防御
  ironclaw_sandbox/        # 新 - 从 src/sandbox 抽出
  ironclaw_secrets/        # 新 - 从 src/secrets 抽出
  x_claw_agent/            # 新 - fork claw-code runtime + commands + hook 扩展点

claw-code/rust/crates/
  api/                     # 作为 path 依赖 or subtree，LLM Provider 抽象
  tools/                   # 作为 path 依赖，内置工具（read/write/bash/edit/grep 等）

desktop-client/
  src/                     # Tauri 壳 + channel
  ironclaw/                # 薄应用层，组装 crates
    src/
      llm/                 # 从 rig_adapter 改为 claw-code api wrapper（~100 行）
      app.rs               # Tauri state 组装
      ipc/                 # chat/approval/files 命令
  src-ui/                  # React 前端
```

---

## 3. Hook Trait 设计（Phase 3 关键）

让 `x_claw_agent` 不直接依赖 ironclaw 的具体安全实现，用 trait 注入：

### 3.1 SafetyHook — DLP 与 prompt injection

```rust
// crates/x_claw_agent/src/hooks.rs
#[async_trait]
pub trait SafetyHook: Send + Sync {
    /// 在把用户输入发给 LLM 前调用
    async fn before_prompt(&self, prompt: &mut String) -> Result<SafetyDecision, SafetyError>;

    /// 在 LLM 输出发回前端前调用
    async fn after_completion(&self, completion: &mut String) -> Result<(), SafetyError>;

    /// 工具调用前（tool injection 防御）
    async fn before_tool_call(
        &self,
        tool_name: &str,
        args: &serde_json::Value,
    ) -> Result<SafetyDecision, SafetyError>;
}

pub enum SafetyDecision { Allow, Redact, Block(String) }
```

ironclaw 端实现：

```rust
// desktop-client/ironclaw/src/safety_adapter.rs
impl SafetyHook for IronclawSafety {
    async fn before_prompt(&self, prompt: &mut String) -> Result<SafetyDecision, SafetyError> {
        let result = ironclaw_safety::detect(prompt)?;
        match result.severity {
            Severity::Block => Ok(SafetyDecision::Block(result.reason)),
            Severity::Redact => { *prompt = result.redacted; Ok(SafetyDecision::Redact) }
            _ => Ok(SafetyDecision::Allow),
        }
    }
    // ...
}
```

### 3.2 SandboxExecutor — 隔离执行

```rust
#[async_trait]
pub trait SandboxExecutor: Send + Sync {
    async fn run_bash(&self, cmd: &str, workdir: &Path) -> Result<BashOutput, SandboxError>;
    async fn read_file(&self, path: &Path) -> Result<Vec<u8>, SandboxError>;
    async fn write_file(&self, path: &Path, data: &[u8]) -> Result<(), SandboxError>;
    async fn fetch(&self, req: HttpRequest) -> Result<HttpResponse, SandboxError>;
}
```

ironclaw 端用 Docker 实现；测试端可用 LocalExecutor 实现。

### 3.3 SecretProvider

```rust
#[async_trait]
pub trait SecretProvider: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<SecretString>, SecretError>;
    async fn list_names(&self) -> Result<Vec<String>, SecretError>;
}
```

### 3.4 ApprovalGate

```rust
#[async_trait]
pub trait ApprovalGate: Send + Sync {
    async fn request(
        &self,
        tool_name: &str,
        args: &serde_json::Value,
    ) -> Result<ApprovalOutcome, ApprovalError>;
}

pub enum ApprovalOutcome { Approved, Denied, ModifiedArgs(serde_json::Value) }
```

由 tauri_channel 实现：request 时通过 DataStream 发 `2:data` 帧给前端，等前端 invoke 回来。

### 3.5 Agent 组装

```rust
// x_claw_agent 对外 API
pub struct Agent<L, S, X, K, A>
where L: LlmProvider, S: SafetyHook, X: SandboxExecutor, K: SecretProvider, A: ApprovalGate,
{ /* ... */ }

impl<L, S, X, K, A> Agent<L, S, X, K, A> {
    pub fn new(llm: L, safety: S, sandbox: X, secrets: K, approval: A) -> Self { /* ... */ }
    pub async fn run_turn(&self, input: UserInput) -> impl Stream<Item = AgentEvent> { /* ... */ }
}
```

ironclaw 组装：

```rust
// desktop-client/ironclaw/src/app.rs
let agent = x_claw_agent::Agent::new(
    ClawCodeApi::new(config),              // LlmProvider
    IronclawSafety::new(policy),           // SafetyHook
    DockerSandbox::new(sandbox_cfg),       // SandboxExecutor
    KeychainSecrets::new(),                // SecretProvider
    TauriApprovalGate::new(channel_tx),    // ApprovalGate
);
```

---

## 4. AI SDK Data Stream Protocol 规范

Phase 1 起后端必须输出此协议。参考 Vercel AI SDK 文档：

| Frame | 用途 | 示例 |
|-------|------|------|
| `0:"text"` | 文本增量 | `0:"Hello, "` |
| `9:{id,name,args}` | 工具调用开始 | `9:{"toolCallId":"call_1","toolName":"write_file","args":{"path":"/tmp/a.md"}}` |
| `a:{toolCallId,result}` | 工具调用结果 | `a:{"toolCallId":"call_1","result":"ok"}` |
| `2:[{...}]` | 任意结构化 data（用于 approval 等自定义事件） | `2:[{"type":"approval_needed","request_id":"req_1",...}]` |
| `d:{finishReason,usage}` | 结束帧 | `d:{"finishReason":"stop","usage":{"promptTokens":100,"completionTokens":50}}` |
| `3:"error"` | 错误 | `3:"rate limited"` |

审批流程：

```
后端                           前端
  │ 2:[{type:"approval_needed", request_id:"r1", tool:"write_file", args:{...}}]
  │ ─────────────────────────→
  │                             SDK data 帧 → ApprovalToolUI 渲染
  │                             用户点"批准"
  │                             invoke("approval_respond", {request_id:"r1", action:"approve"})
  │ ←─────────────────────────
  │ 9:{toolCallId:"r1", ...}    继续执行
  │ ─────────────────────────→
  │ a:{toolCallId:"r1", result}
  │ ─────────────────────────→
```

---

## 5. 数据流：一次完整对话

```
用户输入「查一下百度今日热搜并写成 md」
    ↓
前端 Composer.send()
    ↓
useChatRuntime 调用 TauriTransport.sendMessage()
    ↓
invoke("chat_send", {thread_id, message})
    ↓
ipc/chat.rs → x_claw_agent.run_turn()
    ↓
SafetyHook.before_prompt() ← ironclaw_safety 检查输入
    ↓
LlmProvider.complete() ← claw-code api → DashScope
    ↓ stream delta
AgentEvent::TextDelta → tauri_channel → 0:"..." 帧 → 前端 SDK 渲染
    ↓
LLM 返回 tool_use: web_search
    ↓
AgentEvent::ToolCallStart → 9:{...} 帧 → SDK 自动渲染 WebSearchToolUI
    ↓
SandboxExecutor.fetch() ← ironclaw_sandbox HTTP proxy
    ↓
AgentEvent::ToolCallResult → a:{...} 帧 → SDK 自动填充 result
    ↓
LLM 返回 tool_use: write_file
    ↓
ApprovalGate.request() → 2:[{type:"approval_needed",...}] → 前端 ApprovalToolUI
    ↓ 用户批准
invoke("approval_respond") ← 前端
    ↓
SandboxExecutor.write_file() ← ironclaw_sandbox
    ↓
AgentEvent::TextDelta("已写入 ..." ) → 0:"..." → SDK 渲染
    ↓
AgentEvent::Finish → d:{finishReason:"stop",...} → SDK 结束 turn
```

注意：**前端几乎没有自定义状态管理**。branch/pending/streaming 全由 SDK 内部维护。

---

## 6. 与 claw-code 的同步策略

推荐 **git subtree**（理由：保留完整 git history，双向 patch 友好）：

```bash
# 初始化
git subtree add --prefix=claw-code https://github.com/upstream/claw-code.git main --squash

# Phase 3 抽出 x_claw_agent 时，从 claw-code/rust/crates/runtime 和 commands 复制
# 加上 hooks 抽象层作为 x_claw_agent 的 new code
# 保留引用关系的注释

# 上游更新
git subtree pull --prefix=claw-code https://github.com/upstream/claw-code.git main --squash
# 手动把 upstream 的 runtime/commands 变更 porting 到 x_claw_agent
```

具体 porting 流程见 Phase 3 文档。

---

## 7. 目标指标

| 维度 | 当前 | 目标 |
|------|------|------|
| `rig_adapter.rs` | 2026 行 | 0 |
| `TauriRuntimeProvider.tsx` | 1539 行 | <500 |
| `tauri_channel.rs` | 678 行 | ~300 |
| `agent/*` 混合耦合 | 24041 行 | 拆成 `x_claw_agent`（claw-code port）+ `ironclaw_routines`（原创扩展） |
| rig-core 依赖 | ✓ | 移除 |
| claw-code 上游 rebase 能力 | ❌ | ✅（通过 subtree） |
| LLM Provider 抽象层级 | 错位 | 正确（在 claw-code api 里） |
| SDK 原生能力利用率 | <10% | >80% |
