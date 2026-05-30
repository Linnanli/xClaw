# Tool Call 数据流架构修复方案

> 状态：**已实施（Phase 1 完成）**  
> 创建时间：2026-04-20  
> 实施时间：2026-04-20  
> 涉及模块：`desktop-client/src/tauri_channel.rs`、`desktop-client/ironclaw/`、`desktop-client/ui/`

---

## 1. 问题概述

Desktop Client 的 tool call 渲染**全面失效**：工具展开后显示空内容、web_search 结果含原始 XML、每个工具同时出现两种 UI（可折叠面板 + ☑️ 指示器）。

**根因不是 UI 组件问题，而是整个数据流管线在设计上就不携带 tool call 数据。**

## 2. 当前架构分析

### 2.1 两套 Runtime Provider

| Provider | 位置 | 通信方式 | 协议 |
|----------|------|----------|------|
| `ChatRuntimeProvider` | Admin Backend | HTTP SSE | ✅ Vercel AI Data Stream Protocol |
| `TauriRuntimeProvider` | Desktop Client | Tauri IPC | ❌ 自定义 ChatEvent 协议 |

`ChatRuntimeProvider` 使用 `@assistant-ui/react-ai-sdk` 的 `useChatRuntime` + `AssistantChatTransport`，直接消费标准的 Vercel AI Data Stream Protocol。这条路径**工具渲染正常**。

`TauriRuntimeProvider` 使用 `useExternalStoreRuntime` 手动构建消息，有一套自定义的 `ChatEvent` → `ToolStep` → `toolCalls` 适配层。**这条路径的 tool call 数据在传输过程中全部丢失。**

### 2.2 数据流断裂分析

```
ironclaw Agent
  │
  ├─ TurnToolCall { name, parameters✅, result✅, tool_call_id✅ }
  │
  ├─→ StatusUpdate::ToolStarted { name }                      ← ❌ 无 tool_call_id
  ├─→ StatusUpdate::ToolCompleted { name, success, error, parameters }
  │     └→ ChatEvent::ToolCompleted { name, success, error }  ← ❌ parameters 被 `..` 丢弃
  ├─→ StatusUpdate::ToolResult { name, preview }
  │     └→ ChatEvent::Status { level: "debug" }               ← ❌ 降级为调试日志
  │
  └─→ persist_tool_calls() → DB                               ← ❌ parameters 不写入
```

**前端收到的数据**：
- `tool_started`: 只有 `name`
- `tool_completed`: 只有 `name` + `success` + `error`
- ~~`tool_result`~~: **不存在**（降级为前端忽略的 debug Status）

**最终传给 assistant-ui 的 tool-call part**: `{ toolCallId: "step_0", toolName, args: undefined, result: undefined }`

### 2.3 与 Vercel AI Data Stream Protocol 的对比

| Vercel Protocol 事件 | 携带数据 | ChatEvent 对应 | 数据完整度 |
|---------------------|---------|----------------|-----------|
| `text-start` | messageId | ❌ 无 | — |
| `text-delta` | messageId + delta | `stream_chunk` (content) | ⚠️ 无 messageId |
| `text-end` | messageId | ❌ 无 | — |
| `reasoning-start` | messageId | ❌ 无 | — |
| `reasoning-delta` | messageId + delta | `thinking` (message) | ⚠️ 无 messageId |
| `reasoning-end` | messageId | ❌ 无 | — |
| `tool-input-start` | toolCallId + toolName | `tool_started` (name) | ❌ 无 toolCallId |
| `tool-input-delta` | toolCallId + inputTextDelta | ❌ 无 | ❌ 完全缺失 |
| `tool-input-available` | toolCallId + toolName + input | ❌ 无（parameters 被丢弃） | ❌ 完全缺失 |
| `tool-output-available` | toolCallId + output | ❌ 降级为 debug | ❌ 数据丢失 |
| `tool-output-error` | toolCallId + errorText | `tool_completed.error` | ⚠️ 无 toolCallId |
| `error` | errorText | `error` | ✅ |

### 2.4 其他 IPC 事件的数据丢失

| StatusUpdate 变体 | 丢失字段 | 影响 |
|-------------------|---------|------|
| `ApprovalNeeded` | `parameters`, `allow_always` | 无法展示参数、无"始终允许"按钮 |
| `AuthRequired` | `auth_url`, `setup_url` | 无法触发 OAuth 流程 |
| `JobStarted` | `browse_url` | 无法跳转到沙盒 |

### 2.5 前端双重渲染问题

`response` 事件到达后，同一消息同时携带：
1. `toolCalls` → `convertMessage()` 生成 `tool-call` parts → `ToolFallback` 渲染可折叠面板
2. `metadata.custom.toolSteps` → `ToolStepIndicator` 渲染 ☑️ 时间线

两者描述同一组工具但**数据不同、UI 不同、同时出现**。

## 3. aisdk 参考分析

[lazy-hq/aisdk](https://github.com/lazy-hq/aisdk) 是 Vercel AI SDK 的 Rust 实现，提供：

1. **`VercelUIStream` 枚举**（`src/integrations/vercel_aisdk_ui.rs`）— 完整实现 Vercel AI SDK UI 协议的 13 种事件类型
2. **`into_vercel_ui_stream()`** — 将 language model stream 映射为标准协议事件
3. **`VercelUIMessage` / `VercelUIMessagePart`** — 前端消息格式的反序列化
4. **Axum 集成** — 直接作为 HTTP SSE 端点

关键：aisdk 的 tool call 生命周期是完整的：

```
ToolCallStart → ToolCallDelta (流式 args) → ToolCallAvailable (完整 args)
                                            → 执行工具
                                            → ToolCallEnd (output/error)
```

每个事件都携带 `tool_call_id`，前端可以精确关联。

## 4. 修复方案

### 4.1 三条路径评估

| 路径 | 做法 | 优点 | 缺点 | 推荐 |
|------|------|------|------|------|
| **A: Tauri 内嵌 HTTP** | localhost HTTP 输出 Data Stream SSE | 两种场景统一；直接复用 ChatRuntimeProvider | 桌面 app 开 HTTP 端口；增加攻击面 | ❌ |
| **B: 自定义 TauriChatTransport** | 实现 assistant-ui transport 接口，内部走 Tauri IPC | 进程内零开销；和 ChatRuntimeProvider 共享渲染层 | 需理解 transport 协议 | ✅ **推荐** |
| **C: 修补 ExternalStoreRuntime** | 保持现有 adapter，修复数据传递 | 改动最小 | 仍是自定义 adapter，不复用标准库 | ⚠️ 备选 |

**推荐路径 B**。以下是路径 B 的实现细节，附后备路径 C。

### 4.2 路径 B: TauriChatTransport 方案

#### 核心思路

```
ironclaw Agent
  │
  ├─→ StatusUpdate (扩展: 增加 tool_call_id)
  │     └→ TauriChannel 映射为 VercelUIStream 格式 JSON
  │          └→ Tauri IPC emit("chat-stream", VercelUIStream)
  │
  └─→ respond()
        └→ TauriChannel emit("chat-stream", VercelUIStream::TextEnd)

前端:
  TauriChatTransport
    └→ listen("chat-stream")
       └→ 将 VercelUIStream JSON 转为 ReadableStream
          └→ useChatRuntime 消费（与 ChatRuntimeProvider 相同）
             └→ assistant-ui 自动解析 tool-call parts
                └→ ToolFallback / 注册的 ToolUI 正常渲染
```

#### 改动清单

##### 4.2.1 Rust 层 — ironclaw `StatusUpdate` 扩展（3 变体）

**文件**: `desktop-client/ironclaw/src/channels/channel.rs`

```rust
// 变更 1: ToolStarted 增加 tool_call_id
ToolStarted {
    name: String,
    tool_call_id: String,      // ← 新增
}

// 变更 2: ToolCompleted 已有 parameters，无需改（但现在不会被丢弃了）

// 变更 3: ToolResult 增加 tool_call_id
ToolResult {
    name: String,
    preview: String,
    tool_call_id: String,      // ← 新增
}
```

**影响范围**: 所有发送 `StatusUpdate::ToolStarted` / `ToolResult` 的调用点需补充 `tool_call_id`。经审计，这些调用点都有 `tool_call_id` 可用但当前未传入：
  - `dispatcher.rs` L827 — `tc.id` 可用
  - `thread_ops.rs` L1228 — `pending.tool_call_id` 可用

##### 4.2.2 Rust 层 — TauriChannel 输出 VercelUIStream 格式

**文件**: `desktop-client/src/tauri_channel.rs`

替换 `ChatEvent` 为基于 Vercel 协议的事件格式。不需要依赖 aisdk crate——只需定义相同的 JSON 结构。

```rust
// 新增: VercelUIStream-compatible 事件（仅涉及 tool call 的部分，其余保持 ChatEvent）
// 或者完整替换 ChatEvent 为 VercelUIStream 兼容格式
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ChatStreamEvent {
    // === 文本 ===
    #[serde(rename = "text-delta")]
    TextDelta { id: String, delta: String },

    // === Reasoning ===
    #[serde(rename = "reasoning-delta")]
    ReasoningDelta { id: String, delta: String },

    // === Tool Call ===
    #[serde(rename = "tool-input-start")]
    ToolInputStart {
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        #[serde(rename = "toolName")]
        tool_name: String,
    },
    #[serde(rename = "tool-input-available")]
    ToolInputAvailable {
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        #[serde(rename = "toolName")]
        tool_name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool-output-available")]
    ToolOutputAvailable {
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        output: serde_json::Value,
    },
    #[serde(rename = "tool-output-error")]
    ToolOutputError {
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        #[serde(rename = "errorText")]
        error_text: String,
    },

    // === 非 Vercel 协议的自定义事件（保留） ===
    #[serde(rename = "response")]
    Response { message_id: String, content: String, thread_id: String, source: String },
    #[serde(rename = "error")]
    Error { message: String, code: Option<String> },
    #[serde(rename = "connection_status")]
    ConnectionStatus { connected: bool, message: String },
    #[serde(rename = "approval_needed")]
    ApprovalNeeded {
        thread_id: String,
        request_id: String,
        tool_name: String,
        description: String,
        parameters: serde_json::Value,    // ← 不再丢弃
        allow_always: bool,               // ← 不再丢弃
    },
    // ... 其余自定义事件 ...
}
```

`status_to_event()` 映射更新：

| StatusUpdate | 映射目标 | 数据 |
|---|---|---|
| `ToolStarted { name, tool_call_id }` | `ToolInputStart { tool_call_id, tool_name }` | ✅ 完整 |
| `ToolCompleted { success: true, parameters, .. }` | `ToolInputAvailable { tool_call_id, tool_name, input }` | ✅ 有 args |
| `ToolCompleted { success: false, error, .. }` | `ToolOutputError { tool_call_id, error_text }` | ✅ |
| `ToolResult { name, preview, tool_call_id }` | `ToolOutputAvailable { tool_call_id, output }` | ✅ 有 result |
| `StreamChunk(content)` | `TextDelta { id, delta }` | ✅ |
| `Thinking(msg)` | `ReasoningDelta { id, delta }` | ✅ |

##### 4.2.3 Rust 层 — persist_tool_calls 补全 args

**文件**: `desktop-client/ironclaw/src/agent/thread_ops.rs`

在 `persist_tool_calls()` 的 `summaries` 构建中增加 `args` 字段：

```rust
if !tc.parameters.is_null() {
    let args_str = truncate_preview(&tc.parameters.to_string(), 1000);
    obj["args"] = serde_json::Value::String(args_str);
}
```

##### 4.2.4 前端层 — TauriChatTransport

**文件**: 新增 `desktop-client/ui/src/app/runtime/TauriChatTransport.ts`

实现 assistant-ui 的 transport 接口，将 Tauri IPC 事件桥接为 `useChatRuntime` 可消费的 ReadableStream。

```typescript
import { listen } from '@tauri-apps/api/event';

export class TauriChatTransport {
  // 核心：将 Tauri IPC listen("chat-stream") 事件桥接为
  // assistant-ui transport 协议的 ReadableStream。
  //
  // 每个 VercelUIStream 事件 JSON 按 Data Stream Protocol 编码
  // 为 "d:{json}\n" 格式行，供 assistant-ui 内部 parser 消费。
}
```

具体实现需要研究 `@assistant-ui/react-ai-sdk` 的 `AssistantChatTransport` 源码以确定接口细节。

##### 4.2.5 前端层 — TauriRuntimeProvider 简化

**文件**: `desktop-client/ui/src/app/runtime/TauriRuntimeProvider.tsx`

核心变更：将 `useExternalStoreRuntime` + 手动事件处理 替换为 `useChatRuntime` + `TauriChatTransport`。

删除以下代码块：
- `ToolStep` interface 和 `toolStepsBuffer`（~50 行）
- `handleChatEvent` 中的 `tool_started`/`tool_completed` 手动处理（~80 行）
- `response` 事件中 toolSteps→toolCalls 的转换逻辑（~20 行）
- `convertMessage` 中 toolCalls 到 tool-call parts 的手动映射（~40 行）
- `parsePersistedToolCalls` 的 3 级 fallback 猜测（~50 行）
- `normalizeExternalMessages`（~30 行）

替换为：
```typescript
// 与 ChatRuntimeProvider 相同模式
const transport = new TauriChatTransport({ threadId, ... });
const runtime = useChatRuntime({ transport });
```

##### 4.2.6 前端层 — thread.tsx 消除双重渲染

**文件**: `desktop-client/ui/src/app/components/assistant-ui/thread.tsx`

删除 `ToolStepIndicator` 时间线渲染块。`useChatRuntime` 会自动生成正确的 tool-call parts，由 `ToolFallback` 或注册的 `makeAssistantToolUI` 统一渲染。

### 4.3 路径 C: 备选最小修复方案

如果路径 B 的 transport 接口研究证明过于复杂，退而使用路径 C：

保持 `useExternalStoreRuntime`，但修复数据传递：

1. **Rust 端**：`ChatEvent::ToolCompleted` 增加 `args`、`result`、`tool_call_id` 字段
2. **Rust 端**：新增 `ChatEvent::ToolResult { name, result, tool_call_id }` 事件
3. **前端**：`ToolStep` 增加 `args`/`result`/`toolCallId` 字段
4. **前端**：`response` 事件的 toolCalls 转换包含完整数据
5. **前端**：`onAddToolResult` 回调实现
6. **前端**：ToolStepIndicator 只在流式中（有 running 状态）显示

此路径改动约 ~100 行，5 个文件。

## 5. 决策需确认的问题

| 问题 | 路径 B | 路径 C |
|------|--------|--------|
| 是否采用 Vercel Data Stream Protocol？ | ✅ 是 | ❌ 否（自定义） |
| 是否复用 `ChatRuntimeProvider` 的模式？ | ✅ 共享 `useChatRuntime` | ❌ 保持独立 adapter |
| 是否需要研究 transport 接口？ | ✅ 需要 | ❌ 不需要 |
| 改动范围 | 大（重写 TauriRuntimeProvider） | 小（~100 行修补） |
| 未来维护成本 | 低（标准协议） | 高（自定义协议需持续维护） |
| 是否能复用 aisdk 代码？ | ✅ 映射逻辑直接参考 | ⚠️ 仅学习参考 |

## 6. aisdk 代码提取策略（方案 B 补充）

### 6.1 为什么不整体引入 aisdk crate

aisdk 是一个完整的 Rust LLM SDK，包含 73+ LLM provider、模板引擎、嵌入模型等。ironclaw 已有自己的 LLM 调用层。
我们需要的仅是其 **Vercel UI Stream Protocol 序列化层**。

| 方式 | 新增代码 | 新依赖 | 风险 |
|------|---------|--------|------|
| A. Cargo.toml 加 `aisdk` | 0 行 | aisdk + derive_builder + schemars + parking_lot + aisdk-macros + reqwest-eventsource | 重，拖入整个 SDK |
| **B. 提取协议层（推荐）** | **~320 行** | **0**（serde/serde_json/uuid 已有） | 低，MIT 许可证 |
| C. 从零手写 | ~200 行 | 0 | 中，易写错字段命名/serde 注解 |

### 6.2 aisdk 模块裁剪分析

```
aisdk/
├── core/
│   ├── language_model/    ← LLM 调用（generate_text, stream_text）     ❌ 不需要
│   ├── embedding_model/   ← 嵌入模型                                    ❌ 不需要
│   ├── tools.rs           ← ToolCallInfo, ToolResultInfo               ⚠️ ironclaw 已有
│   ├── messages.rs        ← Message 枚举                               ⚠️ ironclaw 已有
│   ├── client.rs          ← SDK client                                  ❌ 不需要
│   └── provider.rs        ← Provider trait                              ❌ 不需要
├── integrations/
│   ├── vercel_aisdk_ui.rs ← ✅ 提取目标：协议类型 + serde 注解
│   └── axum.rs            ← Axum 集成                                   ❌ 不需要
├── providers/             ← 73+ LLM providers                          ❌ 完全不需要
└── prompt.rs              ← 模板系统                                    ❌ 不需要
```

### 6.3 提取清单

从 `vercel_aisdk_ui.rs` 提取 3 部分到 `desktop-client/src/vercel_ui_protocol.rs`：

| 部分 | 行数 | 用途 |
|------|------|------|
| `VercelUIStream` 枚举 + serde 注解 | ~200 行 | Rust → 前端：序列化为标准 JSON，含 `#[serde(tag = "type", rename_all = "kebab-case")]`、`#[serde(rename = "toolCallId")]` 等精确注解 |
| `VercelUIMessage` / `VercelUIMessagePart` | ~80 行 | 前端 → Rust：反序列化用户消息（含 tool 状态），支持 `output-available`、`output-error`、`output-denied` |
| `VercelUIStreamOptions` + builder | ~40 行 | 控制是否发送 reasoning / start / finish 事件 |

提取的核心价值是 **serde 注解的精确性**（如 `toolCallId` 而非 `tool_call_id`），手写的话一个字段命名错误就导致前端解析失败。

### 6.4 不提取的代码

| 代码 | 原因 |
|------|------|
| `map_language_model_chunk_to_vercel_ui()` | 输入是 `LanguageModelStreamChunkType`，我们的输入是 `StatusUpdate`，需要自己写映射 |
| `into_vercel_ui_stream()` | 依赖 aisdk 内部的 `StreamTextResponse`，不适用 |
| `LanguageModelStreamChunkType` | ironclaw 已有 `StatusUpdate` |
| `ToolCallInfo` / `ToolResultInfo` | ironclaw 已有 `TurnToolCall` |

### 6.5 兼容性分析

**UIMessageChunk / VercelUIStream 协议覆盖的场景：**

| 场景 | 协议事件 | ironclaw StatusUpdate 对应 |
|------|---------|--------------------------|
| 文本流式输出 | `text-start/delta/end` | `StreamChunk` |
| 推理/思考过程 | `reasoning-start/delta/end` | `Thinking` |
| 工具请求（args） | `tool-input-start/delta/available` | `ToolStarted` + `ToolCompleted` |
| 工具结果 | `tool-output-available/error` | `ToolResult` |
| 错误 | `error` | `Error` |

所有主流 LLM 返回内容（文本、reasoning、tool call）均在此协议范围内。

**Tool Call 双向交互：** aisdk 的 `VercelUIMessagePart` 支持完整的 tool 状态反解析：
`input-available` / `approval-requested` / `approval-responded` / `output-available` / `output-error` / `output-denied`。

**HTTP Headers：** 与本方案无关。Headers 是 HTTP transport 的事，我们走 Tauri IPC。

## 7. 实施顺序建议（路径 B）

### Phase 1: 基础设施（Rust 端）

1. 提取 `vercel_ui_protocol.rs` — VercelUIStream 协议类型
2. `StatusUpdate` 扩展 `tool_call_id`（channel.rs + 调用点）
3. `status_to_stream_event()` 新映射函数（StatusUpdate → VercelUIStream）
4. `persist_tool_calls()` 补全 args
5. `cargo build` + `cargo test`

### Phase 2: 前端 Transport

6. 实现 `TauriChatTransport`（listen → ReadableStream）
7. 重写 `TauriRuntimeProvider` 使用 `useChatRuntime`

### Phase 3: 清理 + 验证

8. 删除 `ToolStep` / `toolStepsBuffer` / 双重渲染代码
9. 删除 `parsePersistedToolCalls` 的 fallback 猜测
10. `npx vitest run` + 手动验证 tool UI 渲染
11. 编写新的集成测试

## 8. 附录：aisdk vercel_aisdk_ui.rs 关键代码参考

### VercelUIStream 事件映射函数

aisdk 的 `map_language_model_chunk_to_vercel_ui()` 展示了标准的 model stream → Vercel UI 事件映射模式：

```
LanguageModelStreamChunkType::ToolCallStart(tool)
  → VercelUIStream::ToolInputStart { tool_call_id, tool_name, provider_executed: true }

LanguageModelStreamChunkType::ToolCallDelta { id, delta }
  → VercelUIStream::ToolInputDelta { tool_call_id, input_text_delta }

LanguageModelStreamChunkType::ToolCallAvailable(tool_call)
  → VercelUIStream::ToolInputAvailable { tool_call_id, tool_name, input, provider_executed: true }

LanguageModelStreamChunkType::ToolCallEnd(result_info)
  → VercelUIStream::ToolOutputAvailable { tool_call_id, output }
  或 VercelUIStream::ToolOutputError { tool_call_id, error_text }
```

我们的 `StatusUpdate → ChatStreamEvent` 映射应遵循相同模式。
