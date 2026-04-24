# 01 — 当前三层错位问题分析

> 本文是 `00-overview.md` 中"三层错位"的证据文档。所有行数、文件路径、grep 结果均为真实工程数据，用于支撑 Phase 1-3 的重构决策。

---

## 1. 错位一：LLM 协议层 — rig-core 被当 HTTP 客户端

### 1.1 现象

[`desktop-client/ironclaw/src/llm/rig_adapter.rs`](../../desktop-client/ironclaw/src/llm/rig_adapter.rs) 行数：**2026 行**。

这不是一个正常的 Provider Adapter 应有的体量。作为对比：claw-code 的 `crates/api/src/providers/openai_compat.rs` 整个 OpenAI 兼容实现（支持 OpenAI/xAI/DashScope/Kimi/Ollama）远小于这个数字。

### 1.2 证据：补丁层 `ReasoningPatchClient`

为了让 Qwen / DashScope 的响应能被 rig-core 的 OpenAI parser 接受，我们不得不在 HTTP 层拦截响应、修改 JSON：

```rust
// rig_adapter.rs 节选
struct ReasoningPatchClient { inner: reqwest::Client }

fn patch_reasoning_content(body: &mut serde_json::Value) {
    // 1. 把 reasoning_content 挪到 content
    // 2. 把 finish_reason: null 改成 "stop"
    // 3. 补齐缺失的 object 字段
}
```

**这说明 rig-core 的抽象层级错了**：它假设所有 OpenAI-compat 厂商都 100% 符合 OpenAI spec，没有预留 per-provider 响应后处理扩展点。每新增一个"不太标准"的厂商，我们就要在补丁函数里加一个 if-else。

### 1.3 证据：核心业务不依赖 rig-core 的高级抽象

grep 结果显示，我们只用了 rig-core 的 `completion_request`、`CompletionModel`、`Message` 这些最基础的类型，完全没有用 rig-core 的：

- Agent loop
- Tool registry
- Vector store
- RAG pipeline

**结论**：rig-core 的价值（统一抽象 + Agent 框架）我们一个都没享受到，却承担了它的协议兼容性债务。

### 1.4 代价

- 每出一个新国产模型 → 读文档 → 对比 OpenAI spec → 在 `patch_reasoning_content` 加分支 → 写测试
- 新模型接入时间：**3-5 天**（正常应该 <1 天）
- 维护负担：2026 行补丁代码

---

## 2. 错位二：前端 Runtime 层 — assistant-ui 被当哑渲染容器

### 2.1 现象

[`desktop-client/src-ui/src/app/runtime/TauriRuntimeProvider.tsx`](../../desktop-client/src-ui/src/app/runtime/TauriRuntimeProvider.tsx) 行数：**1539 行**。

使用的是 `useExternalStoreRuntime`，意味着消息/工具/审批的 **所有状态** 都由我们自己维护：

```tsx
// TauriRuntimeProvider.tsx 实际状态
const [pendingApprovals, setPendingApprovals] = useState<PendingApproval[]>([]);
const [isRunning, setIsRunning] = useState(false);
const pendingAssistantId = useRef<string | null>(null);
const activeTurnRef = useRef<boolean>(false);
// ... 还有至少 10 个 useState + useRef
```

### 2.2 证据：SDK 已安装但未用

[`desktop-client/src-ui/package.json`](../../desktop-client/src-ui/package.json) 显示：

```json
{
  "@assistant-ui/react": "^0.12.20",
  "@assistant-ui/react-ai-sdk": "^1.3.15",  // ← 已安装
  "@assistant-ui/react-data-stream": "^0.12.8",  // ← 已安装
  "ai": "^4.0.0"  // ← Vercel AI SDK 已安装
}
```

`@assistant-ui/react-ai-sdk` 提供的 `useChatRuntime` hook 可以直接把 Vercel AI SDK Data Stream 协议接成一个完整 Runtime —— 自动管理消息、工具调用、branch、审批、流式增量。**这个包我们装了但一行没用**。

### 2.3 证据：branch 吞审批 bug 是架构症状

前几天的 UX bug：用户看不到审批卡片，因为审批 tool call 出现在 branch 的 page 2。

- 用 `useExternalStoreRuntime` 时，branch 是我们自己实现的（通过 `pendingAssistantId` + `activeTurnRef`），SDK 的 branch API 我们没接上，所以审批渲染位置和 branch 切换状态是两套逻辑，不同步
- 用 `useChatRuntime` + `makeAssistantToolUI` 时，工具 UI 由 SDK 按 `toolCallId` 注册，branch 切换时 SDK 自动帮你切 toolUI 展示，**根本不会出现"审批卡片在旧 branch 里看不到"**

### 2.4 代价

- 前端状态管理 bug 源源不断（审批/branch/流式 pending 三套状态同步）
- 消息重发/重新生成/编辑等 SDK 原生能力无法直接用
- `TauriRuntimeProvider.tsx` 每次新增能力都在 1500 行文件里改

---

## 3. 错位三：Agent 能力层 — 代码复制 + 失去 rebase 能力

### 3.1 现象

[`desktop-client/ironclaw/src/agent/`](../../desktop-client/ironclaw/src/agent) 目录总计 **24041 行**，主要文件：

| 文件 | 行数 | 对应 claw-code 文件 |
|------|------|-------------------|
| `dispatcher.rs` | 2889 | `runtime/src/dispatcher.rs`（已分歧） |
| `thread_ops.rs` | 2546 | `runtime/src/thread_ops.rs`（已分歧） |
| `session.rs` | 1962 | `runtime/src/session.rs`（已分歧） |
| `agent_loop.rs` | 1824 | `runtime/src/agent_loop.rs`（已分歧） |
| `commands.rs` | - | `commands/src/*`（已分歧） |
| `compaction.rs` | - | `runtime/src/compaction.rs`（已分歧） |
| `context_monitor.rs` | - | `runtime/src/context_monitor.rs`（已分歧） |
| `routine.rs` + `routine_engine.rs` + `scheduler.rs` + `self_repair.rs` | - | **非 claw-code 原生，是我们加的** |

### 3.2 证据：claw-code 有完整 crate 结构

[`claw-code/rust/crates/`](../../claw-code/rust/crates):

```
api/           ← LLM Provider 抽象（Anthropic + OpenAI-compat）
commands/      ← slash commands
runtime/       ← agent loop、session、thread、dispatcher、compaction
tools/         ← 内置工具
plugins/       ← 插件系统
telemetry/     ← 可观测
```

我们复制进 `ironclaw/src/agent/` 之后没有保留 crate 边界，所有东西都在一个 `agent/` 目录下，和 `routine`/`self_repair` 这些 ironclaw 原创代码混在一起。**无法再从上游 git pull**。

### 3.3 代价

- claude-code CLI 每升级一次（plan mode、subagent、compaction 改进），我们都要手动对 diff 重抄一遍
- claw-code 原生能力和 ironclaw 扩展能力耦合，无法独立测试/发布
- Agent 状态管理逻辑和 TauriRuntimeProvider 的前端状态 + rig-core 的 LLM 状态 **三套状态**，边界不清

---

## 4. 叠加效应：为什么"最低级的能力"都有 bug

用户提出的关键问题："为什么基础能力都不完善？"

答案是三层错位叠加：

```
LLM 响应异常（Qwen reasoning_content）
  ↓ 补丁层吞掉异常（rig_adapter.rs 打 JSON 补丁）
  ↓ rig-core 解析出"空消息"
  ↓ TauriRuntimeProvider 收到空消息 → 流式 state 空转
  ↓ 前端用户看到"在转圈但没文字"
  ↓ branch 新建但 tool call 挂在旧 branch
  ↓ 用户看不到审批卡片
  ↓ 用户以为"又卡住了"
```

任何一层如果正确分工，这条链都不会断。**问题不在单点 bug，而在三层错位叠加导致的信号衰减。**

---

## 5. 量化总结

| 指标 | 当前 | 目标（Phase 3 完成后） |
|------|------|--------------------|
| `rig_adapter.rs` 行数 | 2026 | 0（删除） |
| `TauriRuntimeProvider.tsx` 行数 | 1539 | <500 |
| `agent/*` 耦合行数 | 24041（混在一起） | claw-code fork 单独一个 crate + ironclaw 扩展单独一个 crate |
| 新模型接入时间 | 3-5 天 | <1 天 |
| 审批/branch/流式状态同步 bug 数（最近 4 周） | 3 个 | 0（SDK 原生接管） |
| 能从 claude-code 上游 rebase | ❌ | ✅ |

---

## 6. 结论

三层错位是 **架构设计偏离最佳实践**，不是"bug 太多"。打补丁只会让偏离更深。正确做法是分阶段回正：

1. **Phase 1 修前端层**（风险最低收益最快）→ 详见 [`03-phase1-ai-sdk-migration.md`](./03-phase1-ai-sdk-migration.md)
2. **Phase 2 修 LLM 层**（用 claw-code api 替代 rig-core）→ 详见 [`04-phase2-claw-code-api.md`](./04-phase2-claw-code-api.md)
3. **Phase 3 修 Agent 层**（重新获得 rebase 能力）→ 详见 [`05-phase3-agent-extraction.md`](./05-phase3-agent-extraction.md)

每阶段都不破坏 ironclaw 的 5 块安全能力，详见 [`06-safety-preservation.md`](./06-safety-preservation.md)。
