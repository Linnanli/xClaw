# Phase 1：AI SDK 前端 Runtime 迁移方案

> 目标：把 Desktop Client 的聊天 runtime 从手写的 `TauriRuntimeProvider` (1539 行) 迁移到基于 `@assistant-ui/react-ai-sdk` + `TauriChatTransport` 的 `ChatRuntimeProvider` (153 行)，同时保证所有现有功能不丢。
>
> **状态**：方案阶段。本文档不修改任何代码。
>
> **日期**：2026-04-22。

---

## 1. 现状盘点

### 1.1 两套 Provider 并存

| Provider | 文件 | 行数 | 使用者 | 事件源 |
|---|---|---|---|---|
| `TauriRuntimeProvider` | `runtime/TauriRuntimeProvider.tsx` | 1539 | `ChatTabTauri`（**主对话 tab**） | 自建 `ChatEvent` / `subscribe_chat_events` |
| `ChatRuntimeProvider` | `runtime/ChatRuntimeProvider.tsx` | 153 | `ChatTabTauriExperimental`（**实验 tab**） | `TauriChatTransport` → AI SDK v5 `UIMessageChunk` |
| `TauriChatTransport` | `runtime/TauriChatTransport.ts` | 270 | ChatRuntimeProvider | `send_chat_message` + `chat-stream` listen |

### 1.2 ChatRuntimeProvider 目前缺的能力

对比 TauriRuntimeProvider 提供的 API 面：

| 能力 | TauriRuntimeProvider | ChatRuntimeProvider | 缺失度 |
|---|---|---|---|
| 消息发送 + 流式 | ✅ 手写 | ✅ AI SDK 接管 | — |
| DLP 状态 | ✅ DlpContext | ✅ DlpContext | — |
| 工具渲染组件挂载 | ✅ | ✅（已挂全 11 个 Tool UI）| — |
| Approval 线程 id 注入 | ✅ ApprovalThreadIdProvider | ✅ ApprovalThreadIdProvider | — |
| **模型列表加载与切换 (ModelContext)** | ✅ `modelApi.listModels` + `loadModel` + `useModelContext` | ❌ 完全没有 | 🔴 高 |
| **待审批队列 (ApprovalContext)** | ✅ `pendingApprovals: PendingApproval[]` + `useApprovalState` | ❌ | 🔴 高 |
| **历史消息回放** | ✅ `threadApi.loadMessages` → `setMessages` + `parsePersistedToolCalls` + `PersistedUiEvent` | ❌ AI SDK `useChatRuntime` 不提供回放 | 🔴 高 |
| **引擎就绪门控 (engineReadyKey)** | ✅ 启动期 `subscribe_chat_events` + 失败回退 | ❌ | 🟡 中 |
| **外部指令队列 (outboundCommand)** | ✅ `onOutboundCommandHandled` 消费 | ❌ | 🟡 中 |
| **附件序列化 (SerializedAttachment)** | ✅ | ❌ transport 还没接 | 🟡 中 |
| **Routine-triggered 事件** | ✅ `routine_triggered` → 侧边栏通知 | ❌ | 🟢 低 |
| **Job-status 事件** | ✅ `job_status` → Job 面板联动 | ❌ | 🟢 低 |
| **Thinking 阶段事件** | ✅ `thinking` → 加载 UI | ⚠️ AI SDK 有 reasoning 分隔，但 UX 文案未对齐 | 🟢 低 |
| **Connection-status 事件** | ✅ `connection_status` → 离线提示 | ❌ | 🟢 低 |
| **Runtime 错误边界** | ✅ `RuntimeErrorBoundary` (`Component` class) + reset key | ❌ | 🟡 中 |
| **Thread 创建回调 onThreadCreated** | ✅ | ❌ | 🟡 中 |

### 1.3 关键事件通道对齐情况

TauriRuntimeProvider 订阅的 `ChatEvent` 共 **10 个 variant**。AI SDK 接管后，transport 已把前 4 个转换为 `UIMessageChunk`：

| ChatEvent variant | AI SDK 状态 | Phase 1 需要 |
|---|---|---|
| `response` | ✅ `text-delta` / `finish` | — |
| `stream_chunk` | ✅ `text-delta` | — |
| `tool_started` / `tool_completed` | ✅ `tool-input-start` / `tool-output-available` | 需确认 TauriChatTransport 已经映射 |
| `thinking` | ⚠️ 部分 | 映射到 `reasoning-delta` 或独立 UI |
| **`approval_needed`** | ❌ | 独立通道，需自建 `ApprovalContext` 订阅 |
| **`job_status`** | ❌ | 独立通道，不走聊天 transport |
| **`routine_triggered`** | ❌ | 独立通道 |
| **`error`** | ⚠️ AI SDK 有 `onError`，但后端错误码 / 降级文案需保留 | 需 shim |
| **`connection_status`** | ❌ | 独立通道 |

---

## 2. 迁移原则

1. **不破坏现有 `ChatTabTauri` 用户体验**：迁移是渐进的，两个 Provider 可以共存一段时间，直到 `ChatRuntimeProvider` 能力对齐后再把 `ChatTabTauri` 的 import 切过去、废弃旧 Provider。
2. **所有独立业务事件（approval / job / routine / connection / error）不走 chat transport**：它们在架构上就不是 chat 流的一部分，应该通过**独立的 Tauri event bus**直接喂到对应的 Context。
3. **AI SDK 只管 chat stream**，其他一切（模型列表、审批、历史回放、引擎就绪）用**独立的 Context + hook** 围在 `ChatRuntimeProvider` 外层。
4. **禁止把 Context 逻辑塞进 `TauriChatTransport`**：transport 是"聊天流转换器"，不是"应用状态 hub"。
5. **历史回放不用手写**：`useChatRuntime` 支持 `initialMessages`，用后端返回的 `ThreadMessageLike[]` 初始化即可。持久化的 `tool_calls` / `PersistedUiEvent` 逻辑保留但抽成**纯函数**（没有 React 依赖），供两个 Provider 共用。

---

## 3. 分阶段迁移计划

每个阶段独立可交付、可回滚，跑完 e2e 再进下一阶段。

### Phase 1.1 — 共享纯函数抽出（无前端行为变化）

把 TauriRuntimeProvider 内部 3 类无 React 依赖的逻辑抽到 `runtime/shared/`：

- `runtime/shared/persistedEvents.ts`
  - `parsePersistedToolCalls(content)` 从 Provider L146 搬出。
  - `parsePersistedUiEvents(content)` 从 L192 搬出。
  - `serializeAttachment(att)` / `deserializeAttachment(raw)` 从 L494–560 搬出。
- `runtime/shared/chatEventMapping.ts`
  - `ChatEvent` 类型定义（L279）。
  - `mapChatEventToAssistantUpdate(event)` — 把非 stream 事件转为 UI 侧 action（approval/job/routine）。
- `runtime/shared/historyLoader.ts`
  - `loadThreadHistory(threadId): Promise<ThreadMessageLike[]>` — 封装 `threadApi.loadMessages` + persisted events 还原。

**产出**：TauriRuntimeProvider 行数 ~1100（-30%）；ChatRuntimeProvider 行数不变。E2E 验证主对话无回归。

### Phase 1.2 — ChatRuntimeProvider 补齐高优先级 Context

在 `ChatRuntimeProvider` 外层包 3 个新 Context（**不改 transport**）：

1. **`ModelProvider`** (`runtime/contexts/ModelProvider.tsx`)
   - 复用 `modelApi.listModels` / `loadModel`。
   - 暴露 `useModelContext()`：`{ models, selectedModelId, setSelectedModelId, selectedModel, modelsLoading }`。
   - `ChatRuntimeProvider` 通过 props 读取并传给 transport 的 `modelId` / `apiBaseUrl` / `apiKey` refs。
2. **`ApprovalProvider`** (`runtime/contexts/ApprovalProvider.tsx`)
   - 独立订阅 `approval_needed` / `approval_resolved` 事件（新增 Tauri event name，与聊天流解耦）。
   - 暴露 `useApprovalState()`：`{ pendingApprovals, resolveApproval, rejectApproval }`。
3. **`EngineReadyProvider`** (`runtime/contexts/EngineReadyProvider.tsx`)
   - 订阅 `subscribe_chat_events` 结果 + 后端发出的 `engine_ready` 事件。
   - 暴露 `useEngineReady()`：`{ isReady, bootError, retry }`。
   - `ChatRuntimeProvider` 在 `!isReady` 时渲染启动占位，避免 transport 在 IPC 未就绪时发送。

**注意**：这三个 Context **必须**在 `AssistantRuntimeProvider` 外层，以便非聊天 UI（Job 面板、模型选择器、Routine 提示）也能直接读状态，不被 assistant-ui 的 tree 限制。

**产出**：ChatRuntimeProvider 从 153 → ~220 行；新增 3 个 Context ~450 行。E2E 跑 ChatTabTauriExperimental 验证模型切换、审批弹窗、引擎未就绪友好提示。

### Phase 1.3 — 历史回放与外部指令

- `ChatRuntimeProvider` 接收 `threadId` 变化时，调用 `loadThreadHistory(threadId)` 得到 `ThreadMessageLike[]`，传给 `useChatRuntime({ transport, initialMessages })`。
- 新增 `OutboundCommandQueue` context：`{ queue: ChatCommand[], dispatch(cmd), consume(id) }`。transport 在 send 之前先取队列头。

**产出**：ChatRuntimeProvider 可完整替代 TauriRuntimeProvider 的历史/指令能力。

### Phase 1.4 — 侧通道事件 + 错误边界

- Routine / Job / ConnectionStatus：独立 Context + listener，与聊天流彻底解耦。
- `RuntimeErrorBoundary` 原样搬到 `runtime/shared/RuntimeErrorBoundary.tsx`，两个 Provider 都用它包裹。

### Phase 1.5 — 切换 ChatTabTauri + 删除 TauriRuntimeProvider

- 改 `ChatTabTauri` 的 import：`TauriRuntimeProvider` → `ChatRuntimeProvider`。
- 全量 Cypress/Playwright 回归。
- 删除 `runtime/TauriRuntimeProvider.tsx`、`runtime/__tests__/TauriRuntimeProvider.*.test.tsx`、`ChatTabTauriExperimental.tsx`（合并到 `ChatTabTauri`）。
- 后端相关 `subscribe_chat_events` / `ChatEvent` 重新审视：原 10 个 variant 中的 chat 相关保留，独立业务事件拆出独立命令。

---

## 4. 后端配合清单

Phase 1.2 要求后端提供 **3 个独立事件通道**（目前全被塞进 `chat-stream`）：

| 事件 | 现状 | 目标 |
|---|---|---|
| Approval 请求 | `ChatEvent::ApprovalNeeded` 混在 `chat-stream` | 独立 Tauri event `approval-event`，payload `{ action: 'needed'|'resolved', request_id, ... }` |
| Job 状态 | `ChatEvent::JobStatus` | 独立 event `job-status` |
| Routine 触发 | `ChatEvent::RoutineTriggered` | 独立 event `routine-event` |
| Engine 就绪信号 | 当前靠 `subscribe_chat_events` 返回值 | 独立 event `engine-status`（idle / ready / error） |

后端改动不涉及 `dispatcher.rs`，只涉及 `tauri_channel.rs` 的事件路由。**与 D-5 deferral 兼容**。

---

## 5. 工作量与风险

| 阶段 | 预估工作量 | 风险 |
|---|---|---|
| 1.1 纯函数抽出 | 1 天 | 低。没有行为变化，单元测试覆盖即可。 |
| 1.2 三大 Context | 2–3 天 | 中。需要后端配合拆事件通道。 |
| 1.3 历史回放 | 1–2 天 | 中。`initialMessages` 与 AI SDK 的 reasoning/tool-call 段保留需要验证。 |
| 1.4 侧通道 + 错误边界 | 1 天 | 低。 |
| 1.5 切换 + 删除 | 1–2 天（含回归） | 高。全量回归必跑，否则主对话有可能回退。 |

**总计**：~1.5 周（含 QA）。

**风险红线**：

1. **AI SDK v5 的 tool-call UI 状态机与 `ToolStep` 不 1:1**。实验 tab 的 tool renderer 已经用了 SDK 的协议（`tool-input-start` / `tool-output-available`），但历史数据里的 `PersistedToolCall` 需要一套映射器。Phase 1.3 的 `loadThreadHistory` 必须验证这个转换不丢数据。
2. **`useChatRuntime` 不支持中途替换 transport**。模型切换时 apiKey/apiBaseUrl 变了，目前 transport 用 getter 闭包读最新值——这条设计保留，**不要**试图用"重建 transport"的方式支持模型切换。
3. **approval/job/routine 的独立事件通道必须在 Phase 1.2 就切到后端独立 emit**，否则前端两次从 `chat-stream` 解析会出竞态。

---

## 6. 开始条件检查

在启动 Phase 1.1 之前必须确认：

- [ ] `TauriChatTransport` 目前已经把 `tool_started` / `tool_completed` 正确映射到 AI SDK chunk（否则 1.1 前还要补 transport）。
- [ ] 后端 `tauri_channel.rs` 确认可以新增独立 event name（不破坏现有订阅者）。
- [ ] 现有 Cypress / Playwright 主对话回归用例齐备（否则 1.5 切换阶段没有安全网）。
- [ ] 决定 **Phase 1 期间 `ChatTabTauriExperimental` 是否继续存在**：推荐存在直到 1.4 结束，1.5 合并删除。

---

## 7. 下一步（建议落到 session memory）

- 本方案是 Phase 1 **总纲**。下一次开工时从 Phase 1.1 开始，每个阶段结束更新本文件的 Phase 1.x 对应段落，记录实际落地情况。
- 如果 Phase 1.2 的后端事件通道拆分阻塞，优先做 1.1 + 1.4（不依赖后端），把前端重构做完。
- D-5 保持 deferred 不受影响——Phase 1 只动 `desktop-client/src-ui/` + `tauri_channel.rs`，不碰 `dispatcher.rs`/`agent_loop.rs`。
