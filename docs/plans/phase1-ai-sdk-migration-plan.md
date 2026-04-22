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

### 1.3 关键事件通道对齐情况（已更正）

> **事实核查**：原稿把 approval/job/routine/connection_status 描述成需要"独立事件通道"，这是基于旧印象的错误假设。
> 代码证据：后端 9 处 `app_handle.emit()` 调用（`engine.rs`/`main.rs`/`tauri_channel.rs`/`approval_polling.rs`/`ipc/chat.rs`/`ipc/jobs.rs`）**全部** target `"chat-stream"`；
> 前端 2 处 `listen()` 调用（`TauriRuntimeProvider.tsx:997` + `useEngineReady.tsx:27`）也都订阅 `"chat-stream"`。
> `ChatEvent` TS 类型仍存在，但只是 `VercelStreamEvent.data-custom.data` 的 inner shape，不是独立事件。
> `useEngineReady` 已是"多订阅者共享 `chat-stream` + 按 `data-custom.data.type` 过滤"的工作范式。

TauriRuntimeProvider 订阅的 `ChatEvent` 共 **10 个 variant**，全部通过 `VercelStreamEvent` 的 `data-custom` variant 复用同一个 `chat-stream` 通道。AI SDK 接管后，transport 已把前 4 个转换为 AI SDK 原生 chunk：

| ChatEvent variant | 传输方式 | AI SDK 状态 | Phase 1 需要 |
|---|---|---|---|
| `response` | `text-delta` / `finish` | ✅ 原生 | — |
| `stream_chunk` | `text-delta` | ✅ 原生 | — |
| `tool_started` / `tool_completed` | `tool-input-start` / `tool-input-available` / `tool-output-available` / `tool-output-error` | ✅ 原生，**transport 已 identity 透传**（`TauriChatTransport.ts:79-89`） | — |
| `thinking` | **`reasoning-delta`**（`tauri_channel.rs:177` `StatusUpdate::Thinking → VercelUIStream::ReasoningDelta`） | ✅ 原生 | —（文案若不一致再调） |
| `approval_needed` | `data-custom.data.type = 'approval_needed'` | ❌ Context 未建 | **前端**建 `ApprovalContext`，订阅同一个 `chat-stream` 按 inner type 分流 |
| `job_status` | `data-custom.data.type = 'job_status'` | ❌ Context 未建 | **前端**建 Job Context，同上 |
| `routine_triggered` | `data-custom.data.type = 'routine_triggered'` | ❌ Context 未建 | **前端**建 Routine Context，同上 |
| `error` | `VercelStreamEvent.error` + AI SDK `onError` | ⚠️ 需 shim | 保留后端错误码 / 降级文案的 shim |
| `connection_status` | `data-custom.data.type = 'connection_status'` | ✅ `useEngineReady` 已消费 | 保留该 hook，无需新增 |

> **事实核查更正**：原稿把 `thinking` 列为"⚠️ 部分"、把 `tool_*` 列为"需确认"——都是过时判断。代码证据显示它们都已经是 AI SDK 原生 chunk，**不需要 shim**。真正需要 Context 分流的只剩 approval_needed / job_status / routine_triggered 三类。

---

## 2. 迁移原则

1. **不破坏现有 `ChatTabTauri` 用户体验**：迁移是渐进的，两个 Provider 可以共存一段时间，直到 `ChatRuntimeProvider` 能力对齐后再把 `ChatTabTauri` 的 import 切过去、废弃旧 Provider。
2. **非流业务事件（approval / job / routine / connection / error）复用现有的 `chat-stream` 单通道**：它们已经以 `data-custom.data.type = ...` 的形式多路复用在同一个 Tauri event 里；Phase 1 **不拆通道**，而是在前端多个 Context 独立订阅 `chat-stream`，各自按 `data-custom.data.type` 过滤自己关心的 inner type（范式参考 `useEngineReady.tsx`）。
3. **AI SDK 只管转换 stream chunk**（`text-delta` / `tool-*` / `finish`）；`data-custom` variant 的非流事件由**独立 Context** 在 `ChatRuntimeProvider` 外层自行订阅同一条 `chat-stream` 处理，不通过 AI SDK 传递。
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
  - `ChatEvent` 类型定义（L279）——**仅作为 `data-custom.data` 的 inner shape 类型**，不是独立事件源。
  - `dispatchDataCustom(event)` — 根据 `event.payload.data.type` 分发到 approval/job/routine/error Context。**不包含** thinking / tool_* （它们已是 AI SDK 原生 chunk，不经过这个分发器）。
- `runtime/shared/historyLoader.ts`
  - `loadThreadHistory(threadId): Promise<UIMessage[]>` — 封装 `threadApi.getMessages` + persisted events 还原。
  - **关键工作量**：DB 三段式 `user → tool_calls → assistant` 需展开为 AI SDK 的 `UIMessage.parts` discriminated union（`text` / `tool-${name}` / `reasoning` / `data-${kind}`）；`routineTriggerCount` / `dlpStats` / `attachments` 没有原生 `UIMessage` 字段，统一落到 `message.metadata` 或 `data-*` parts。参考现有 `TauriRuntimeProvider.tsx:870-985` 的合并逻辑，但结果结构不同。

**产出**：TauriRuntimeProvider 行数 ~1100（-30%）；ChatRuntimeProvider 行数不变。E2E 验证主对话无回归。

### Phase 1.2 — ChatRuntimeProvider 补齐高优先级 Context

在 `ChatRuntimeProvider` 外层包 3 个新 Context（**不改 transport**）：

1. **`ModelProvider`** (`runtime/contexts/ModelProvider.tsx`)
   - 复用 `modelApi.listModels` / `loadModel`。
   - 暴露 `useModelContext()`：`{ models, selectedModelId, setSelectedModelId, selectedModel, modelsLoading }`。
   - `ChatRuntimeProvider` 通过 props 读取并传给 transport 的 `modelId` / `apiBaseUrl` / `apiKey` refs。
2. **`ApprovalProvider`** (`runtime/contexts/ApprovalProvider.tsx`)
   - `listen<VercelStreamEvent>('chat-stream', …)` 自行订阅，过滤 `payload.type === 'data-custom' && payload.data?.type === 'approval_needed'`。
   - 暴露 `useApprovalState()`：`{ pendingApprovals, resolveApproval, rejectApproval }`。
   - **不依赖后端改动**——事件已经在 `chat-stream` 里，后端 `approval_polling.rs` 已 emit。
3. **`EngineReadyProvider`** (`runtime/contexts/EngineReadyProvider.tsx`)
   - **直接复用现有 `useEngineReady.tsx` 实现**（已是 `chat-stream` + `data-custom.data.type === 'connection_status'` 的消费者），提升为 Context 即可。
   - 暴露 `useEngineReady()`：`{ isReady, bootError, retry }`。
   - `ChatRuntimeProvider` 在 `!isReady` 时渲染启动占位，避免 transport 在 IPC 未就绪时发送。

**注意**：这三个 Context **必须**在 `AssistantRuntimeProvider` 外层，以便非聊天 UI（Job 面板、模型选择器、Routine 提示）也能直接读状态，不被 assistant-ui 的 tree 限制。

**多订阅者性能注意**：多个 Context 都 `listen('chat-stream')` 意味着同一个事件会被 Tauri 递送多次（每个订阅者一份）。`useEngineReady` 已经在这么做，实测无感。若后续出现热路径问题，可在 Phase 1.4 抽一个 `ChatStreamBus` 单例做内部 fan-out，再让 Context 订阅该 bus——但 Phase 1.2 不必做这个优化。

**产出**：ChatRuntimeProvider 从 153 → ~220 行；新增 3 个 Context ~450 行。E2E 跑 ChatTabTauriExperimental 验证模型切换、审批弹窗、引擎未就绪友好提示。

**前置依赖 ⚠️**：`ChatTabTauriExperimental` 目前在 `src-ui/` 没有任何入口引用（MainApp 只 import `ChatTabTauri`）。**Phase 1.2 开始前必须先在 MainApp 加个 dev-only 切换**（feature flag / URL param / 隐藏 tab）让 Experimental 渲染起来，否则"实验 tab 并行验证"这条安全网是空的。这个切换在 Phase 1.5 合并时一并拆掉。

**Experimental 依赖缺口清单**（必须在 Phase 1.2 新 Context 完成后立即接入 Experimental，否则 E2E 验证只能覆盖 DLP + 单模型）：

| 能力 | Experimental 当前状态 | 修复方式 |
|---|---|---|
| 自定义模型 `apiBaseUrl` / `apiKey` 透传 | ❌ 只传 `threadId` + `modelId` | Phase 1.2 ModelProvider 完成后，Experimental 从 `useModelContext()` 读并透入 `ChatRuntimeProvider` |
| ApprovalContext 注入 | ❌ 按自述"hook 读默认空数组、FloatingApprovalBanner 空转" | Phase 1.2 ApprovalProvider 建成后，Experimental 外层包 `ApprovalProvider` |
| `onThreadCreated` 回调 | ❌ 按自述"新 Runtime 暂不支持运行时创建线程回调" | Phase 1.3 OutboundCommandQueue 同批 |

### Phase 1.3 — 历史回放与外部指令

**现状**：`ChatRuntimeProvider` 目前接收 `threadId` 但**完全没接历史回放**——`useChatRuntime({ transport })` 调用无 `initialMessages`，也无 `useEffect` 监听 threadId 重载（`ChatRuntimeProvider.tsx:77-119`）。切换会话时旧消息不会出现。

- 实现 `historyLoader.loadThreadHistory(threadId)` → `UIMessage[]`（见 1.1）。
- `ChatRuntimeProvider` 新增 `useEffect`：threadId 变化时 `await loadThreadHistory` 后调用 `runtime.setMessages(…)` 或利用 AI SDK `useChat` 的 `initialMessages` + `key` 重建方式。**两种路径都需验证**：`useChatRuntime` 的 runtime 对象是否暴露 `setMessages` API。
- 新增 `OutboundCommandQueue` context：`{ queue: ChatCommand[], dispatch(cmd), consume(id) }`。transport 在 send 之前先取队列头。
- 新增 `onThreadCreated` 回调 shim：当后端返回新 threadId 时通知外层（Experimental 自述"新 Runtime 暂不支持运行时创建线程回调"的缺口在这里补）。

**产出**：ChatRuntimeProvider 可完整替代 TauriRuntimeProvider 的历史/指令能力。

**风险红线**：这个阶段的工作量主体是"`TauriMessage → UIMessage.parts` 映射器"，不是"接管 API"。估时 1–2 天效验过小，**下调为 2–3 天**。

### Phase 1.4 — 侧通道事件 + 错误边界

- Routine / Job：新增独立 Context，各自 `listen('chat-stream')` 并按 `data-custom.data.type` 过滤自己关心的 inner type（不新增 Tauri event name）。
- `RuntimeErrorBoundary` 原样搬到 `runtime/shared/RuntimeErrorBoundary.tsx`，两个 Provider 都用它包裹。
- （可选）若多订阅者的 fan-out 变复杂，可抽 `ChatStreamBus` 单例（一处 listen，内部用 `EventTarget` 向多个消费者分发）。**默认不做**。

### Phase 1.5 — 切换 ChatTabTauri + 删除 TauriRuntimeProvider

- 改 `ChatTabTauri` 的 import：`TauriRuntimeProvider` → `ChatRuntimeProvider`。
- 全量 Cypress/Playwright 回归。
- 删除 `runtime/TauriRuntimeProvider.tsx`、`runtime/__tests__/TauriRuntimeProvider.*.test.tsx`、`ChatTabTauriExperimental.tsx`（合并到 `ChatTabTauri`）。
- 后端 `chat-stream` / `ChatEvent` 保持不变：`VercelStreamEvent + data-custom` 的载波协议是稳定合约。

---

## 4. 后端配合清单

> **已更正**：原稿要求后端把 approval/job/routine/engine-status 拆到独立 Tauri event name。代码核查后这条 ask **整个作废**。

**Phase 1 不需要任何后端改动。** 证据：

- 后端 9 处 `emit()` 全部 target `"chat-stream"`（`engine.rs` / `main.rs` / `tauri_channel.rs` / `approval_polling.rs` / `ipc/chat.rs` / `ipc/jobs.rs`）。
- 前端 2 处 `listen()`（`TauriRuntimeProvider.tsx:997` + `useEngineReady.tsx:27`）都订阅 `"chat-stream"`。
- 非流事件已经以 `VercelStreamEvent.data-custom.data.type` 的形式在同一通道内区分。

Phase 1.2 只在前端建 Context、订阅同一条 `chat-stream`、按 `data-custom.data.type` 过滤即可。后端 `tauri_channel.rs` / `dispatcher.rs` 在 Phase 1 内**零改动**。**与 D-5 deferral 天然兼容**。

---

## 5. 工作量与风险

| 阶段 | 预估工作量 | 风险 |
|---|---|---|
| 1.1 纯函数抽出 | 1 天 | 低。没有行为变化，单元测试覆盖即可。 |
| 1.2 三大 Context | **1–2 天**（原 2–3 天，移除后端依赖后下调） | 低–中。纯前端，参照 `useEngineReady` 范式。 |
| 1.3 历史回放 | **2–3 天**（原 1–2 天，上调因为真正工作是 TauriMessage→UIMessage.parts 映射） | **中–高**。DB 三段式合并 + discriminated union parts + metadata 嵌套 三个事交汇在这里。 |
| 1.4 侧通道 + 错误边界 | 1 天 | 低。 |
| 1.5 切换 + 删除 | 1–2 天（含回归） | 高。全量回归必跑，否则主对话有可能回退。 |

**总计**：~1–1.5 周（含 QA）。最大不确定性落在 1.3 的映射器；其他阶段都有明确参照实现。

**风险红线**：

1. **AI SDK v5 的 tool-call UI 状态机与 `ToolStep` 不 1:1**。实验 tab 的 tool renderer 已经用了 SDK 的协议（`tool-input-start` / `tool-output-available`），但历史数据里的 `PersistedToolCall` 需要一套映射器。Phase 1.3 的 `loadThreadHistory` 必须验证这个转换不丢数据。
2. **`useChatRuntime` 不支持中途替换 transport**。模型切换时 apiKey/apiBaseUrl 变了，目前 transport 用 getter 闭包读最新值——这条设计保留，**不要**试图用"重建 transport"的方式支持模型切换。
3. **多个 Context 订阅同一个 `chat-stream`**：Tauri 的 `listen` 会为每个订阅者独立分发事件，`useEngineReady` 已证明该模式可行。若未来事件吞吐压力变大再引入 `ChatStreamBus` fan-out；Phase 1 不做。
4. **`useChatRuntime` 的历史注入 API 不清**。AI SDK v5 `useChat` 有 `initialMessages`，但 `useChatRuntime` 包装后的 runtime 对象是否暴露 `setMessages` / `replaceMessages` 待确认。如无则只能靠改 `key` 整个重建 runtime（每次会话切换都关 transport 连接），与风险 2 冲突。Phase 1.3 第一步就是贴源码核查这点。

---

## 6. 开始条件检查

在启动 Phase 1.1 之前必须确认：

- [x] ~~`TauriChatTransport` 目前已经把 `tool_started` / `tool_completed` 正确映射到 AI SDK chunk~~ — **已确认：`TauriChatTransport.ts:79-89` identity 透传 `tool-input-*` / `tool-output-*`，`thinking` 后端已映射为 `reasoning-delta`（`tauri_channel.rs:177`）**。
- [x] ~~后端 `tauri_channel.rs` 确认可以新增独立 event name~~ — **已确认：Phase 1 不需要后端改动**，`chat-stream` + `data-custom` 单通道是稳定合约。
- [ ] **在 MainApp 为 `ChatTabTauriExperimental` 提供 dev-only 切换入口**（feature flag / URL param）——否则 Phase 1.2+ 的 E2E 验证无路径运行。
- [ ] 现有 Cypress / Playwright 主对话回归用例齐备（否则 1.5 切换阶段没有安全网）。
- [ ] 决定 **Phase 1 期间 `ChatTabTauriExperimental` 是否继续存在**：推荐存在直到 1.4 结束，1.5 合并删除。

---

## 7. 下一步（建议落到 session memory）

- 本方案是 Phase 1 **总纲**。下一次开工时从 Phase 1.1 开始，每个阶段结束更新本文件的 Phase 1.x 对应段落，记录实际落地情况。
- Phase 1 全程**不需要后端改动**（已修订），所以没有后端阻塞风险。
- D-5 保持 deferred 不受影响——Phase 1 **只动 `desktop-client/src-ui/`**，完全不碰 Rust 侧。
