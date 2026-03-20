# 迁移到 Tauri IPC 通信方案

## 问题

当前 Desktop Client 使用 HTTP API (`http://localhost:38080/api/chat/send`) 进行聊天通信,存在以下问题:

1. **依赖外部服务**: 必须先启动 IronClaw 服务器
2. **端口管理**: 需要管理端口 38080,可能冲突
3. **安全风险**: HTTP 端口暴露在本地网络
4. **架构不合理**: Tauri 应用应该使用 IPC,而不是 HTTP

## 解决方案: 使用 Tauri IPC

### 方案概述

将聊天功能从 HTTP API 迁移到 Tauri 命令,使用 Tauri 事件系统替代 SSE。

```
┌──────────────────────────────────────┐
│      Desktop Client (Tauri)          │
│                                      │
│  前端 (React)                        │
│    │                                 │
│    │ invoke('send_chat_message')    │
│    ▼                                 │
│  Rust 后端 (Commands)                │
│    │                                 │
│    │ window.emit('chat-event')      │
│    ▼                                 │
│  前端 (React)                        │
│                                      │
└──────────────────────────────────────┘
```

### 实施步骤

#### 步骤 1: 创建聊天 Tauri 命令

在 `desktop-client/src/commands.rs` 中添加:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageRequest {
    pub thread_id: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageResponse {
    pub message_id: String,
    pub success: bool,
}

/// 发送聊天消息
#[tauri::command]
pub async fn send_chat_message(
    state: tauri::State<'_, CommandState>,
    thread_id: String,
    content: String,
) -> Result<SendMessageResponse> {
    // 调用 API 客户端
    let response = state.api_client.send_message(SendMessageRequest {
        thread_id,
        content,
    }).await?;
    
    Ok(response)
}

/// 订阅聊天事件
#[tauri::command]
pub async fn subscribe_chat_events(
    state: tauri::State<'_, CommandState>,
    window: tauri::Window,
) -> Result<()> {
    let api_client = state.api_client.clone();
    
    tokio::spawn(async move {
        // 连接到 SSE 端点
        let mut events = api_client.subscribe_events().await;
        
        while let Some(event) = events.next().await {
            // 将事件发送到前端
            if let Err(e) = window.emit("chat-event", &event) {
                eprintln!("Failed to emit chat event: {}", e);
            }
        }
    });
    
    Ok(())
}

/// 取消订阅聊天事件
#[tauri::command]
pub async fn unsubscribe_chat_events(
    state: tauri::State<'_, CommandState>,
) -> Result<()> {
    // 实现取消订阅逻辑
    Ok(())
}
```

#### 步骤 2: 注册命令

在 `desktop-client/src/main.rs` 中注册新命令:

```rust
tauri::Builder::default()
    .manage(state)
    .invoke_handler(tauri::generate_handler![
        // ... 现有命令
        send_chat_message,
        subscribe_chat_events,
        unsubscribe_chat_events,
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
```

#### 步骤 3: 更新前端 Hook

创建新的 `useAiChatTauri.ts`:

```typescript
/**
 * AI Chat Hook - 使用 Tauri IPC
 */

import { useState, useCallback, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { useDlpScan } from './useDlpScan';

interface Message {
  id: string;
  role: 'user' | 'assistant';
  content: string;
}

interface ChatEvent {
  type: 'response' | 'thinking' | 'status' | 'error';
  data: any;
}

interface UseAiChatOptions {
  threadId: string;
}

export function useAiChatTauri(options: UseAiChatOptions) {
  const { threadId } = options;
  const [messages, setMessages] = useState<Message[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const { scanUserInput } = useDlpScan();

  // 订阅聊天事件
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;

    const setupEventListener = async () => {
      // 监听聊天事件
      unlisten = await listen<ChatEvent>('chat-event', (event) => {
        const { type, data } = event.payload;

        switch (type) {
          case 'response':
            // 添加 AI 响应消息
            setMessages(prev => [...prev, {
              id: data.message_id,
              role: 'assistant',
              content: data.content,
            }]);
            setIsLoading(false);
            break;

          case 'thinking':
            // 显示思考状态
            setIsLoading(true);
            break;

          case 'status':
            // 更新状态
            console.log('Status:', data);
            break;

          case 'error':
            // 显示错误
            setError(data.message);
            setIsLoading(false);
            break;
        }
      });

      // 订阅聊天事件
      await invoke('subscribe_chat_events');
    };

    setupEventListener();

    return () => {
      if (unlisten) {
        unlisten();
      }
      invoke('unsubscribe_chat_events');
    };
  }, []);

  // 发送消息
  const sendMessage = useCallback(
    async (content: string) => {
      try {
        setIsLoading(true);
        setError(null);

        // DLP 扫描
        const dlpResult = await scanUserInput(content);
        if (dlpResult.was_blocked) {
          throw new Error(dlpResult.block_reason || 'Message blocked by DLP');
        }

        // 使用脱敏后的内容
        const sanitizedContent = dlpResult.sanitized_content;

        // 添加用户消息
        const userMessage: Message = {
          id: `msg-${Date.now()}`,
          role: 'user',
          content: sanitizedContent,
        };
        setMessages(prev => [...prev, userMessage]);

        // 发送消息到后端
        await invoke('send_chat_message', {
          threadId,
          content: sanitizedContent,
        });

      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
        setIsLoading(false);
      }
    },
    [threadId, scanUserInput]
  );

  return {
    messages,
    isLoading,
    error,
    sendMessage,
  };
}
```

#### 步骤 4: 更新组件使用新 Hook

在 `ChatTabWithAiSdk.tsx` 中:

```typescript
import { useAiChatTauri } from '../hooks/useAiChatTauri';

export function ChatTabWithAiSdk() {
  const chat = useAiChatTauri({
    threadId: selectedConversation || '',
  });

  // 其他代码保持不变
}
```

### 优点

1. **无需外部服务**: 不再依赖 HTTP 端口
2. **更安全**: 所有通信通过 IPC,不暴露端口
3. **更快速**: IPC 比 HTTP 更快
4. **类型安全**: Tauri 自动处理序列化
5. **更简单**: 不需要管理 SSE 连接

### 迁移检查清单

- [ ] 创建聊天 Tauri 命令
  - [ ] `send_chat_message`
  - [ ] `subscribe_chat_events`
  - [ ] `unsubscribe_chat_events`
- [ ] 注册命令到 Tauri
- [ ] 创建新的 `useAiChatTauri` Hook
- [ ] 更新组件使用新 Hook
- [ ] 测试聊天功能
- [ ] 移除旧的 HTTP API 调用
- [ ] 更新文档

### 测试计划

1. **单元测试**: 测试 Tauri 命令
2. **集成测试**: 测试前后端通信
3. **E2E 测试**: 测试完整的聊天流程

### 回滚计划

如果迁移出现问题,可以快速回滚:

1. 保留旧的 `useAiChat` Hook
2. 在组件中切换回旧 Hook
3. 移除新的 Tauri 命令

### 时间估算

- **步骤 1-2**: 2-3 小时 (创建和注册命令)
- **步骤 3**: 2-3 小时 (创建新 Hook)
- **步骤 4**: 1 小时 (更新组件)
- **测试**: 2-3 小时
- **总计**: 1-2 天

## 下一步

1. 先完成当前的端口修复 (已完成)
2. 创建 Tauri 命令 (本方案)
3. 测试和验证
4. 移除 HTTP API 依赖

## 参考资料

- `desktop-client/ARCHITECTURE_EVOLUTION.md` - 完整架构演进方案
- [Tauri 命令文档](https://tauri.app/v1/guides/features/command/)
- [Tauri 事件文档](https://tauri.app/v1/guides/features/events/)
