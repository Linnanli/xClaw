# Tauri IPC 通信机制

> 最后更新: 2025-01-XX

本文档详细介绍 Desktop Client 中 Tauri IPC 的设计和使用。

## 什么是 Tauri IPC?

Tauri IPC (Inter-Process Communication) 是 Tauri 提供的前后端通信机制,允许前端 JavaScript 调用后端 Rust 函数。

### 核心概念

```
前端 (JavaScript)          后端 (Rust)
     │                        │
     │  invoke('command')     │
     ├───────────────────────►│
     │                        │
     │  ◄───────────────────┤
     │  return result         │
     │                        │
     │  listen('event')       │
     │  ◄───────────────────┤
     │  emit('event')         │
```

## 通信方式

### 1. 命令调用 (Command)

前端调用后端函数,等待返回结果。

**前端**:
```typescript
import { invoke } from '@tauri-apps/api/core';

const result = await invoke<string>('greet', {
  name: 'World',
});
console.log(result); // "Hello, World!"
```

**后端**:
```rust
#[tauri::command]
fn greet(name: String) -> String {
    format!("Hello, {}!", name)
}
```

### 2. 事件系统 (Event)

后端主动向前端发送事件,前端监听事件。

**后端发送**:
```rust
window.emit("my-event", MyEventPayload {
    message: "Hello from Rust!".to_string(),
})?;
```

**前端监听**:
```typescript
import { listen } from '@tauri-apps/api/event';

const unlisten = await listen<MyEventPayload>('my-event', (event) => {
  console.log('Received:', event.payload.message);
});

// 清理
unlisten();
```

## Desktop Client 中的应用

### 聊天功能

#### 发送消息 (Command)

**前端**:
```typescript
const response = await invoke<SendMessageResponse>('send_chat_message', {
  threadId: 'thread-123',
  content: 'Hello, AI!',
});
```

**后端**:
```rust
#[tauri::command]
pub async fn send_chat_message(
    state: State<'_, CommandState>,
    thread_id: String,
    content: String,
) -> Result<SendMessageResponse, String> {
    // 调用 IronClaw API
    let response = state.api_client
        .send_message(thread_id, content)
        .await
        .map_err(|e| e.to_string())?;
    
    Ok(response)
}
```

#### 接收事件 (Event)

**后端订阅 SSE 并转发**:
```rust
#[tauri::command]
pub async fn subscribe_chat_events(
    state: State<'_, CommandState>,
    window: Window,
) -> Result<(), String> {
    let api_client = state.api_client.clone();
    
    tokio::spawn(async move {
        let mut events = api_client.subscribe_sse().await;
        
        while let Some(event) = events.next().await {
            // 转发到前端
            window.emit("chat-event", event).ok();
        }
    });
    
    Ok(())
}
```

**前端监听**:
```typescript
const unlisten = await listen<ChatEvent>('chat-event', (event) => {
  switch (event.payload.type) {
    case 'response':
      addMessage(event.payload);
      break;
    case 'thinking':
      setThinking(event.payload.message);
      break;
    case 'error':
      showError(event.payload.message);
      break;
  }
});
```

### DLP 扫描

**前端**:
```typescript
const result = await invoke<SanitizationResult>('scan_user_input', {
  content: '我的身份证号是 330326199408015618',
});

console.log(result.sanitized_content);
// "我的身份证号是 330************618"
```

**后端**:
```rust
#[tauri::command]
pub async fn scan_user_input(
    state: State<'_, CommandState>,
    content: String,
) -> Result<SanitizationResult, String> {
    let dlp = state.dlp_manager.lock().await;
    dlp.scan(&content)
        .await
        .map_err(|e| e.to_string())
}
```

## 类型安全

### TypeScript 类型定义

```typescript
// 请求类型
interface SendMessageRequest {
  threadId: string;
  content: string;
}

// 响应类型
interface SendMessageResponse {
  message_id: string;
  thread_id: string;
  content: string;
}

// 事件类型
type ChatEvent =
  | { type: 'response'; message_id: string; content: string }
  | { type: 'thinking'; message: string }
  | { type: 'error'; message: string };
```

### Rust 类型定义

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct SendMessageRequest {
    pub thread_id: String,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SendMessageResponse {
    pub message_id: String,
    pub thread_id: String,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ChatEvent {
    #[serde(rename = "response")]
    Response {
        message_id: String,
        content: String,
    },
    #[serde(rename = "thinking")]
    Thinking {
        message: String,
    },
    #[serde(rename = "error")]
    Error {
        message: String,
    },
}
```

## 错误处理

### 后端错误处理

```rust
#[tauri::command]
pub async fn risky_operation() -> Result<String, String> {
    // 使用 ? 操作符传播错误
    let result = some_operation()
        .await
        .map_err(|e| format!("Operation failed: {}", e))?;
    
    Ok(result)
}
```

### 前端错误处理

```typescript
try {
  const result = await invoke('risky_operation');
  console.log('Success:', result);
} catch (error) {
  console.error('Error:', error);
  toast.error(error as string);
}
```

## 性能优化

### 1. 避免频繁调用

**❌ 不好**:
```typescript
// 每次输入都调用
onChange={(e) => {
  invoke('validate_input', { value: e.target.value });
}}
```

**✅ 好**:
```typescript
// 使用防抖
const debouncedValidate = useMemo(
  () => debounce((value: string) => {
    invoke('validate_input', { value });
  }, 300),
  []
);

onChange={(e) => {
  debouncedValidate(e.target.value);
}}
```

### 2. 批量操作

**❌ 不好**:
```typescript
// 多次调用
for (const item of items) {
  await invoke('process_item', { item });
}
```

**✅ 好**:
```typescript
// 批量处理
await invoke('process_items', { items });
```

### 3. 异步并发

**❌ 不好**:
```typescript
// 串行执行
const result1 = await invoke('operation1');
const result2 = await invoke('operation2');
```

**✅ 好**:
```typescript
// 并发执行
const [result1, result2] = await Promise.all([
  invoke('operation1'),
  invoke('operation2'),
]);
```

## 安全考虑

### 1. 输入验证

**后端验证**:
```rust
#[tauri::command]
pub async fn send_message(content: String) -> Result<(), String> {
    // 验证输入
    if content.is_empty() {
        return Err("Content cannot be empty".to_string());
    }
    
    if content.len() > 10000 {
        return Err("Content too long".to_string());
    }
    
    // 处理
    Ok(())
}
```

### 2. 权限控制

**Tauri 配置**:
```json
{
  "tauri": {
    "allowlist": {
      "all": false,
      "fs": {
        "scope": ["$APPDATA/*"]
      },
      "http": {
        "scope": ["http://localhost:*"]
      }
    }
  }
}
```

### 3. 敏感数据处理

**不要在事件中传递敏感数据**:
```rust
// ❌ 不好
window.emit("user-data", UserData {
    password: "secret123",  // 敏感数据
})?;

// ✅ 好
window.emit("user-data", UserData {
    user_id: "123",
    // 不包含敏感数据
})?;
```

## 调试技巧

### 1. 查看命令调用

**前端**:
```typescript
const result = await invoke('my_command', { arg: 'value' });
console.log('Command result:', result);
```

**后端**:
```rust
#[tauri::command]
fn my_command(arg: String) -> String {
    tracing::debug!("Command called with arg: {}", arg);
    // ...
}
```

### 2. 查看事件流

**前端**:
```typescript
await listen('my-event', (event) => {
  console.log('Event received:', event.payload);
});
```

**后端**:
```rust
tracing::debug!("Emitting event: {:?}", payload);
window.emit("my-event", payload)?;
```

### 3. 使用 Tauri DevTools

```bash
# 启动开发模式
cargo tauri dev

# 打开 DevTools
# 在应用中按 F12
```

## 最佳实践

### 1. 命名约定

- 命令名使用 snake_case: `send_chat_message`
- 事件名使用 kebab-case: `chat-event`
- 类型名使用 PascalCase: `ChatEvent`

### 2. 错误处理

- 后端返回 `Result<T, String>`
- 前端使用 try-catch 捕获
- 提供清晰的错误消息

### 3. 类型定义

- 前后端类型保持一致
- 使用 TypeScript 和 Rust 类型检查
- 定义清晰的接口

### 4. 文档注释

**Rust**:
```rust
/// 发送聊天消息
///
/// # 参数
///
/// * `thread_id` - 线程 ID
/// * `content` - 消息内容
///
/// # 返回
///
/// 返回消息 ID
#[tauri::command]
pub async fn send_chat_message(
    thread_id: String,
    content: String,
) -> Result<String, String> {
    // ...
}
```

**TypeScript**:
```typescript
/**
 * 发送聊天消息
 *
 * @param threadId - 线程 ID
 * @param content - 消息内容
 * @returns 消息 ID
 */
async function sendMessage(
  threadId: string,
  content: string,
): Promise<string> {
  return invoke('send_chat_message', { threadId, content });
}
```

## 常见问题

### Q: 如何传递复杂对象?

A: 使用 JSON 序列化,Tauri 会自动处理:

```typescript
await invoke('complex_command', {
  data: {
    nested: {
      value: 123,
    },
  },
});
```

### Q: 如何处理大数据传输?

A: 考虑使用文件系统或流式传输:

```rust
// 写入文件
std::fs::write(path, data)?;

// 返回文件路径
Ok(path)
```

### Q: 事件监听器会泄漏吗?

A: 需要手动清理:

```typescript
useEffect(() => {
  const unlisten = await listen('my-event', handler);
  
  return () => {
    unlisten();  // 清理
  };
}, []);
```

## 相关文档

- [架构总览](overview.md) - 系统架构
- [快速参考](../../TAURI_IPC_QUICK_REFERENCE.md) - API 快速参考
- [Tauri 官方文档](https://tauri.app/v1/guides/features/command/)

---

**注意**: Tauri IPC 是 Desktop Client 的核心通信机制,理解其工作原理对开发至关重要。
