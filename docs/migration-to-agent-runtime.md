# 改造路径：ChatRuntimeProvider → TauriRuntimeProvider

> 目标：将对话流从"无状态 LLM 代理"升级为"完整 Agent 运行时"，获得 Skills、工具调用、多步推理等能力。

---

## 当前状态

```
ChatTabTauri
  └─ ChatRuntimeProvider (路径 A，活跃)
       └─ AssistantChatTransport → Admin Backend /api/chat/completions
            └─ llm.complete() → 单次文本补全，无 Agent 能力

TauriRuntimeProvider (路径 B，代码就绪，未接入)
  └─ useExternalStoreRuntime → Tauri IPC → 内嵌 Agent
       └─ 完整 agentic loop：skills + 工具 + 多步推理 + 审批
```

---

## 改造分 4 个阶段

### Phase 1：前端切换（最小可用）

改动范围：仅前端，零 Rust 改动

1. `ChatTabTauri.tsx` 中将 `ChatRuntimeProvider` 替换为 `TauriRuntimeProvider`
2. 传入 `threadId` 和 `onThreadCreated` props（当前 ChatRuntimeProvider 不需要这些，TauriRuntimeProvider 需要）
3. 统一 `useDlpState` 的导入源（从 `ChatRuntimeProvider` 改为 `TauriRuntimeProvider`）

```diff
- import { ChatRuntimeProvider, useDlpState } from '../../runtime/ChatRuntimeProvider';
+ import { TauriRuntimeProvider, useDlpState } from '../../runtime/TauriRuntimeProvider';

- <ChatRuntimeProvider key={...} apiUrl={apiUrl} modelId={modelConfig.selectedModelId}>
+ <TauriRuntimeProvider
+   key={selectedThreadId ?? 'new'}
+   threadId={selectedThreadId ?? null}
+   onThreadCreated={(tid) => onThreadSelect?.(tid)}
+ >
    <Thread />
- </ChatRuntimeProvider>
+ </TauriRuntimeProvider>
```

验证点：
- [ ] 发送消息 → Agent 处理 → 收到回复
- [ ] thinking 事件正确渲染思考链
- [ ] DLP 扫描正常拦截
- [ ] 线程创建和历史加载正常

此阶段暂时丢失的能力：
- ❌ 前端模型选择（ModelSelector 无效，用引擎默认模型）
- ❌ DLP redacted stats 展示（TauriRuntimeProvider 的 DlpContext 缺少 redactedStats）

---

### Phase 2：补齐模型动态切换

改动范围：Rust 后端 + 前端

IronClaw 的 LLM provider 已有 `set_model()` 方法（`LlmProvider` trait），支持运行时切换模型。

#### 2a. Rust 侧：send_chat_message 支持 model_id

```rust
// desktop-client/src/ipc/chat.rs
#[tauri::command]
pub async fn send_chat_message(
    state: State<'_, EngineState>,
    thread_id: String,
    content: String,
    model_id: Option<String>,  // 新增：可选模型 ID
) -> Result<SendMessageResponse, String> {
    let state = state.get()?;

    // 如果指定了模型，通过 set_model() 切换
    if let Some(ref model) = model_id {
        if let Err(e) = state.llm_provider.set_model(model) {
            tracing::warn!(model = %model, error = %e, "Failed to switch model, using default");
        }
    }

    // ... 其余逻辑不变
}
```

需要确认：
- `AppState` 中是否暴露了 `llm_provider` 引用
- `set_model()` 是否线程安全（当前实现用 `RwLock`，是安全的）

#### 2b. 前端侧：TauriRuntimeProvider 传递 model_id

```typescript
// TauriRuntimeProvider.tsx 的 onNew 回调中
await invoke<SendMessageResponse>('send_chat_message', {
  threadId: tid,
  content,
  modelId: modelIdRef.current,  // 新增
});
```

#### 2c. 前端侧：TauriRuntimeProvider 接受 modelId prop

```typescript
interface TauriRuntimeProviderProps {
  children: ReactNode;
  threadId: string | null;
  onThreadCreated?: (threadId: string) => void;
  modelId?: string;  // 新增
}
```

#### 2d. ChatTabTauri 接入 ModelSelector

```tsx
<TauriRuntimeProvider
  threadId={selectedThreadId ?? null}
  onThreadCreated={(tid) => onThreadSelect?.(tid)}
  modelId={modelConfig.selectedModelId}
>
```

验证点：
- [ ] ModelSelector 切换模型 → 下一条消息用新模型
- [ ] 模型不存在时 graceful fallback（用默认模型）
- [ ] 契约测试更新：send_chat_message 新增 model_id 参数

---

### Phase 3：补齐 DLP 完整体验 + 流式输出

改动范围：仅前端

#### 3a. DLP Context 补齐 redactedStats

当前 TauriRuntimeProvider 的 DlpContext 只有 `blocked/blockReason/clearBlock`，
缺少 `redactedStats/clearRedacted/onBlocked/onRedacted`。

```typescript
// TauriRuntimeProvider.tsx
interface DlpState {
  blocked: boolean;
  blockReason: string | null;
  clearBlock: () => void;
  redactedStats: SanitizationStats | null;  // 补齐
  clearRedacted: () => void;                // 补齐
  onBlocked: (reason: string) => void;      // 补齐
  onRedacted: (stats: SanitizationStats) => void;  // 补齐
}
```

在 `onNew` 的 DLP 扫描逻辑中，当 `scanUserInput` 返回脱敏结果时，调用 `onRedacted(stats)`。

#### 3b. 流式输出（stream_chunk 事件）

当前 TauriRuntimeProvider 的 `handleChatEvent` 只处理 `response`（完整消息），
不处理 `stream_chunk`（流式片段）。用户体验是"等很久然后一次性出现"。

```typescript
case 'stream_chunk': {
  setIsRunning(true);
  if (!pendingAssistantId.current) {
    const tempId = `stream-${Date.now()}`;
    pendingAssistantId.current = tempId;
    setMessages(prev => [...prev, {
      id: tempId,
      role: 'assistant',
      content: event.content,
      timestamp: Date.now(),
    }]);
  } else {
    const tempId = pendingAssistantId.current;
    setMessages(prev => prev.map(m =>
      m.id === tempId
        ? { ...m, content: m.content + event.content }
        : m
    ));
  }
  break;
}
```

#### 3c. 工具调用事件展示

处理 `tool_started` / `tool_completed` / `approval_needed` 事件，
在消息列表中展示工具执行状态。

```typescript
case 'tool_started': {
  // 在当前 assistant 消息中追加工具状态
  break;
}
case 'approval_needed': {
  // 弹出审批对话框，调用 ic_approve_tool / ic_deny_tool
  break;
}
```

验证点：
- [ ] 流式输出有打字机效果
- [ ] DLP 脱敏后显示 DlpWarningBanner
- [ ] 工具调用过程可见
- [ ] 审批弹窗正常工作

---

### Phase 4：清理 + 文档

#### 4a. 决定 ChatRuntimeProvider 的去留

两个选项：

| 选项 | 做法 | 适用场景 |
|------|------|---------|
| 保留为降级模式 | 引擎未就绪时自动 fallback 到路径 A | 引擎启动慢或启动失败时仍可聊天 |
| 完全移除 | 删除 ChatRuntimeProvider + chat_proxy 相关代码 | 简化架构，减少维护成本 |

建议：保留，作为 `EngineState !== Ready` 时的降级方案。

#### 4b. 更新架构文档

- `docs/ai-chat-architecture.md` 标注路径 B 为主路径
- 更新数据流图

#### 4c. 清理未使用代码

- 如果不保留路径 A：删除 `ChatRuntimeProvider.tsx`、`useAiChatTauri.ts`（如果不再需要）
- 如果保留：标注为 fallback，加注释说明

---

## 改动影响矩阵

| 文件 | Phase | 改动类型 |
|------|-------|---------|
| `src-ui/src/app/components/tabs/ChatTabTauri.tsx` | 1 | 替换 Provider |
| `src-ui/src/app/runtime/TauriRuntimeProvider.tsx` | 2,3 | 加 modelId prop, DLP 补齐, stream_chunk |
| `src-ui/src/app/components/assistant-ui/thread.tsx` | 1 | useDlpState 导入源 |
| `src/ipc/chat.rs` | 2 | send_chat_message 加 model_id 参数 |
| `src/ipc/chat_tests.rs` | 2 | 更新测试 |
| `tests/tauri_command_contract_tests.rs` | 2 | 验证参数变更兼容性 |
| `src-ui/src/app/runtime/ChatRuntimeProvider.tsx` | 4 | 保留或删除 |
| `docs/ai-chat-architecture.md` | 4 | 更新 |

---

## 风险点

| 风险 | 影响 | 缓解措施 |
|------|------|---------|
| 引擎启动慢，用户等待 | 打开应用后无法立即聊天 | Phase 4a 保留 ChatRuntimeProvider 作为降级 |
| set_model() 并发安全 | 多条消息同时发送时模型切换竞争 | RwLock 已保证安全；或改为 per-request model |
| stream_chunk 事件频率高 | React 频繁 setState 导致性能问题 | 用 requestAnimationFrame 节流，或 useRef 缓冲 |
| DlpContext 接口变更 | thread.tsx 等消费方需要适配 | Phase 1 统一导入源时一并处理 |
| send_chat_message 参数变更 | 旧前端代码不兼容 | model_id 为 Optional，向后兼容 |

---

## 预估工作量

| Phase | 工作量 | 前置依赖 |
|-------|--------|---------|
| Phase 1：前端切换 | 0.5 天 | 无 |
| Phase 2：模型动态切换 | 1-2 天 | Phase 1 |
| Phase 3：DLP + 流式 + 工具 | 2-3 天 | Phase 1 |
| Phase 4：清理文档 | 0.5 天 | Phase 1-3 |
| 总计 | 4-6 天 | |

Phase 1 可以独立上线验证，Phase 2 和 Phase 3 可以并行开发。
