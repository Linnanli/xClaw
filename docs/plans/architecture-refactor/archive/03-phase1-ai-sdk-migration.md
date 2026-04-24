# Phase 1 — AI SDK 协议迁移

> **时长**：1-2 周
> **风险**：低（前端改动可 feature flag，后端只加不删）
> **收益**：前端 1539 → ~400 行；审批/branch/流式 bug 根治
> **前置条件**：无（npm 依赖已就绪）

---

## 目标

把前端 Runtime 从 `useExternalStoreRuntime`（自己维护所有状态）切换到 `useChatRuntime`（SDK 管理所有状态），后端输出 Vercel AI SDK Data Stream Protocol。

**不动**：LLM 层（rig-core 保留）、Agent 层（`agent/*` 不重构）、5 块安全能力。

---

## 非目标

- ❌ 不删 rig-core（留给 Phase 2）
- ❌ 不抽 `x_claw_agent` crate（留给 Phase 3）
- ❌ 不升级 AI SDK v4 → v5（留给 Phase 1.5 或 2）

---

## 依赖确认

当前 [`package.json`](../../desktop-client/src-ui/package.json) 已有：

- ✅ `@assistant-ui/react": "^0.12.20"`
- ✅ `@assistant-ui/react-ai-sdk": "^1.3.15"`
- ✅ `@assistant-ui/react-data-stream": "^0.12.8"`
- ✅ `ai: "^4.0.0"`（Vercel AI SDK）

**不需要新增任何 npm 依赖**。

---

## 实施步骤

### Step A — 后端：定义 DataStreamFrame 类型（~0.5 天）

文件：`desktop-client/src/data_stream.rs`（新建）

```rust
use serde::Serialize;

#[derive(Serialize)]
#[serde(untagged)]
pub enum DataStreamFrame {
    Text { text: String },                           // 0:"text"
    ToolCall { id: String, name: String, args: serde_json::Value }, // 9:{...}
    ToolResult { id: String, result: serde_json::Value },           // a:{...}
    Data { items: Vec<serde_json::Value> },          // 2:[...]
    Finish { reason: String, usage: Usage },         // d:{...}
    Error { message: String },                       // 3:"..."
}

impl DataStreamFrame {
    pub fn encode(&self) -> String {
        match self {
            Self::Text { text } => format!("0:{}\n", serde_json::to_string(text).unwrap()),
            Self::ToolCall { id, name, args } => {
                let payload = serde_json::json!({
                    "toolCallId": id, "toolName": name, "args": args
                });
                format!("9:{}\n", payload)
            }
            Self::ToolResult { id, result } => {
                format!("a:{}\n", serde_json::json!({"toolCallId": id, "result": result}))
            }
            Self::Data { items } => format!("2:{}\n", serde_json::json!(items)),
            Self::Finish { reason, usage } => {
                format!("d:{}\n", serde_json::json!({"finishReason": reason, "usage": usage}))
            }
            Self::Error { message } => format!("3:{}\n", serde_json::to_string(message).unwrap()),
        }
    }
}

#[derive(Serialize)]
pub struct Usage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
}
```

**测试**：`tests/data_stream_encoding_tests.rs`，断言每种帧编码成协议规定字符串。

---

### Step B — 后端：tauri_channel.rs 输出 DataStream 帧（~2 天）

当前 [`tauri_channel.rs`](../../desktop-client/src/tauri_channel.rs) 678 行，监听各种 `AgentEvent` 后 emit 自定义 payload。

重构方向：保留事件监听，改为：
1. 把 `AgentEvent` 映射到 `DataStreamFrame`
2. 编码成协议字符串
3. 通过 Tauri `Channel<String>`（Tauri 2.0）推给前端

```rust
// tauri_channel.rs 核心片段（重构后）
async fn forward_agent_events(
    mut rx: broadcast::Receiver<AgentEvent>,
    channel: tauri::ipc::Channel<String>,
) {
    while let Ok(ev) = rx.recv().await {
        let frames = map_agent_event_to_frames(ev);
        for f in frames {
            if let Err(e) = channel.send(f.encode()) {
                tracing::warn!("stream channel closed: {e}");
                return;
            }
        }
    }
}

fn map_agent_event_to_frames(ev: AgentEvent) -> Vec<DataStreamFrame> {
    match ev {
        AgentEvent::TextDelta(t) => vec![DataStreamFrame::Text { text: t }],
        AgentEvent::ToolCallStart { id, name, args } => vec![
            DataStreamFrame::ToolCall { id, name, args }
        ],
        AgentEvent::ToolCallResult { id, result } => vec![
            DataStreamFrame::ToolResult { id, result }
        ],
        AgentEvent::ApprovalNeeded { request_id, tool, args } => vec![
            DataStreamFrame::Data { items: vec![serde_json::json!({
                "type": "approval_needed",
                "request_id": request_id,
                "tool": tool,
                "args": args,
            })] }
        ],
        AgentEvent::Finish { reason, usage } => vec![
            DataStreamFrame::Finish { reason, usage: usage.into() }
        ],
        AgentEvent::Error(msg) => vec![DataStreamFrame::Error { message: msg }],
        // ... 其他事件
    }
}
```

**回归测试**：触发一条含工具调用的 agent turn，捕获 channel 输出字符串，断言包含 `0:` / `9:` / `a:` / `d:` 帧。

---

### Step C — 后端：chat_send 返回 channel（~1 天）

文件：`desktop-client/src/ipc/chat.rs`（修改）

```rust
#[tauri::command]
pub async fn chat_send(
    state: State<'_, AppState>,
    channel: tauri::ipc::Channel<String>,  // ← 新参数
    request: ChatSendRequest,
) -> Result<ChatSendResponse, String> {
    let rx = state.agent.subscribe_events(&request.thread_id);
    tauri::async_runtime::spawn(forward_agent_events(rx, channel));
    state.agent.send_message(request).await.map_err(|e| e.to_string())?;
    Ok(ChatSendResponse { stream_id: request.thread_id })
}
```

**契约测试**：`tests/tauri_command_contract_tests.rs` 加入新签名验证。

---

### Step D — 前端：实现 TauriTransport（~1 天）

文件：`desktop-client/src-ui/src/app/runtime/TauriTransport.ts`（新建，~80 行）

```typescript
import { invoke, Channel } from "@tauri-apps/api/core";
import type { ChatTransport, ChatRequestOptions } from "ai";

export class TauriTransport implements ChatTransport {
  async sendMessages(options: ChatRequestOptions): Promise<Response> {
    const channel = new Channel<string>();

    // 构造 ReadableStream，把 Tauri Channel 转成 web stream
    const stream = new ReadableStream({
      start(controller) {
        channel.onmessage = (chunk) => {
          controller.enqueue(new TextEncoder().encode(chunk));
        };
      },
    });

    await invoke("chat_send", {
      channel,
      request: {
        thread_id: options.chatId,
        messages: options.messages,
      },
    });

    return new Response(stream, {
      headers: { "Content-Type": "text/plain; charset=utf-8" },
    });
  }

  async reconnectToStream(): Promise<Response | null> {
    return null;  // 暂不支持重连，Phase 1.5 再加
  }
}
```

**单测**：mock `invoke` + Channel，验证 sendMessages 正确构造请求。

---

### Step E — 前端：切换 Runtime（~1 天）

文件：`desktop-client/src-ui/src/app/runtime/TauriRuntimeProvider.tsx`（重写）

```tsx
import { useChatRuntime } from "@assistant-ui/react-ai-sdk";
import { AssistantRuntimeProvider } from "@assistant-ui/react";
import { TauriTransport } from "./TauriTransport";

const transport = new TauriTransport();

export function TauriRuntimeProvider({ children, threadId }: Props) {
  const runtime = useChatRuntime({
    api: "",  // 不用 HTTP，用 transport
    transport,
    chatId: threadId,
  });

  return (
    <AssistantRuntimeProvider runtime={runtime}>
      {children}
    </AssistantRuntimeProvider>
  );
}
```

**删除的代码**：原 1539 行中 ~1100 行（所有 `pendingApprovals` / `pendingAssistantId` / `activeTurnRef` / `restoredApprovals` / 手动 branch 逻辑）。

**保留的代码**：
- 线程列表管理（`threadList` API，可能仍需自定义）
- 会话恢复（`restoreFromHistory`，但逻辑简化）

---

### Step F — 前端：审批 UI 改为 ToolUI 注册（~1.5 天）

文件：`desktop-client/src-ui/src/app/components/tool-ui/approval-tool-ui.tsx`（新建）

```tsx
import { makeAssistantToolUI } from "@assistant-ui/react";
import { invoke } from "@tauri-apps/api/core";

export const ApprovalToolUI = makeAssistantToolUI<ApprovalArgs, ApprovalResult>({
  toolName: "approval_request",
  render: ({ args, status, result }) => {
    if (status === "running") {
      return (
        <ApprovalCard
          tool={args.tool}
          params={args.params}
          onApprove={() => invoke("approval_respond", {
            request_id: args.request_id,
            action: "approve",
          })}
          onDeny={() => invoke("approval_respond", {
            request_id: args.request_id,
            action: "deny",
          })}
        />
      );
    }
    if (status === "complete") {
      return <ApprovalResultBanner result={result} />;
    }
    return null;
  },
});
```

注册点：`App.tsx` 或专门的 `ToolUIRegistry.tsx`：

```tsx
<AssistantRuntimeProvider runtime={runtime}>
  <ApprovalToolUI />
  <WebSearchToolUI />
  <WriteFileToolUI />
  {children}
</AssistantRuntimeProvider>
```

**关键变化**：
- 原 `FloatingApprovalBanner`（临时 hack）可删除
- 审批卡片由 SDK 按 `toolCallId` 挂到正确 branch，**彻底解决 branch 吞审批 bug**

---

### Step G — 后端：把 approval_needed 转成 tool call 帧（~0.5 天）

原方案用 `2:data` 自定义事件发 approval。**更优方案**：直接把 approval 建模为一个"虚拟工具调用"，用标准 `9:` 帧：

```rust
AgentEvent::ApprovalNeeded { request_id, tool, args } => vec![
    DataStreamFrame::ToolCall {
        id: request_id.clone(),
        name: "approval_request".to_string(),
        args: serde_json::json!({
            "request_id": request_id,
            "tool": tool,
            "params": args,
        }),
    }
]
```

审批响应触发 `a:` 帧（ToolResult）自动关闭该 UI。**SDK 全自动管理生命周期，我们不写一行状态代码**。

---

### Step H — 冒烟测试 + 回归（~1 天）

新建 `desktop-client/src-ui/e2e/ai-sdk-migration.spec.ts`：

1. 发送 "查百度热搜并写成 md"
2. 期望：文本流式渲染 → 看到 web_search ToolUI → 看到 approval_request ToolUI → 点批准 → 看到 write_file 结果
3. 全程不应出现"审批卡片不可见"

手动回归清单：
- [ ] 普通文本对话
- [ ] 工具调用（web_search / web_fetch / read_file / write_file / bash）
- [ ] 审批流（write_file 触发）
- [ ] Branch 切换（新建对话分支后审批仍可见）
- [ ] 会话恢复（刷新后历史可见）
- [ ] DLP 脱敏（输入含 API key 被打码）
- [ ] 错误场景（LLM 429 → 前端看到错误提示）

---

## 验收标准

- [ ] `TauriRuntimeProvider.tsx` 行数 ≤ 500
- [ ] `useExternalStoreRuntime` 从生产代码删除（保留测试 mock 可以）
- [ ] 审批回归测试通过
- [ ] Branch 吞审批 bug 无法复现
- [ ] `cargo build -p desktop-client` 0 错 0 警
- [ ] 前端 `npx tsc --noEmit` 0 错
- [ ] 所有安全冒烟测试通过（DLP/沙箱/审批）

---

## 回滚策略

Step A-C（后端）：纯新增，不影响旧代码路径，无需回滚。

Step D-G（前端）：用 feature flag 切换：

```tsx
const USE_AI_SDK = import.meta.env.VITE_USE_AI_SDK === "true";

export function TauriRuntimeProvider(props) {
  return USE_AI_SDK ? <NewProvider {...props} /> : <LegacyProvider {...props} />;
}
```

出问题 → 设 env = false 重启。Phase 1 结束稳定后删除 flag 和 LegacyProvider。

---

## 估时

| Step | 任务 | 估时 |
|------|------|------|
| A | DataStreamFrame 类型 + 编码测试 | 0.5d |
| B | tauri_channel 输出协议 | 2d |
| C | chat_send channel 参数 | 1d |
| D | TauriTransport 类 | 1d |
| E | TauriRuntimeProvider 重写 | 1d |
| F | Approval ToolUI | 1.5d |
| G | approval → tool call 帧 | 0.5d |
| H | 冒烟 + 回归 | 1d |
| — | 缓冲 | 1.5d |
| **合计** | | **10d（两周）** |
