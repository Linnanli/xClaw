# Tauri IPC 快速参考

## 快速开始

### 1. 在组件中使用

```typescript
import { useAiChatTauri } from '@hooks/useAiChatTauri';

const chat = useAiChatTauri({
  threadId: 'thread-123',
});

// 发送消息
await chat.sendMessage('Hello!');

// 访问状态
console.log(chat.messages);      // 消息列表
console.log(chat.isLoading);     // 加载状态
console.log(chat.isConnected);   // 连接状态
```

### 2. 完整示例

```typescript
export function ChatComponent() {
  const [input, setInput] = useState('');
  
  const chat = useAiChatTauri({
    threadId: selectedThread,
    onError: (error) => toast.error(error),
    onStatusChange: (status) => console.log(status),
  });

  const handleSend = async () => {
    if (!input.trim()) return;
    await chat.sendMessage(input);
    setInput('');
  };

  return (
    <div>
      {/* 连接状态 */}
      <div>
        {chat.isConnected ? '✅ Connected' : '⚠️  Disconnected'}
      </div>

      {/* 消息列表 */}
      {chat.messages.map((msg) => (
        <div key={msg.id}>
          <strong>{msg.role}:</strong> {msg.content}
        </div>
      ))}

      {/* 思考状态 */}
      {chat.thinkingMessage && (
        <div>💭 {chat.thinkingMessage}</div>
      )}

      {/* 输入框 */}
      <input
        value={input}
        onChange={(e) => setInput(e.target.value)}
        onKeyPress={(e) => e.key === 'Enter' && handleSend()}
        disabled={chat.isLoading}
      />
    </div>
  );
}
```

## API 参考

### useAiChatTauri

#### 参数

```typescript
interface UseAiChatTauriOptions {
  threadId: string;                    // 必需: 会话 ID
  onError?: (error: string) => void;   // 可选: 错误回调
  onStatusChange?: (status: string) => void; // 可选: 状态回调
}
```

#### 返回值

```typescript
{
  // 状态
  messages: Message[];           // 消息列表
  isLoading: boolean;            // 加载状态
  isConnected: boolean;          // 连接状态
  error: string | null;          // 错误信息
  thinkingMessage: string | null; // 思考消息

  // 操作
  sendMessage: (content: string) => Promise<void>;
  clearError: () => void;
  clearMessages: () => void;
}
```

## 事件类型

### ChatEvent

```typescript
type ChatEvent =
  | { type: 'response'; message_id: string; content: string; thread_id: string }
  | { type: 'thinking'; message: string }
  | { type: 'status'; message: string; level: string }
  | { type: 'error'; message: string; code?: string }
  | { type: 'connection_status'; connected: boolean; message: string };
```

## Tauri 命令

### send_chat_message

发送聊天消息

```typescript
const response = await invoke<SendMessageResponse>('send_chat_message', {
  threadId: 'thread-123',
  content: 'Hello, AI!',
});
```

### subscribe_chat_events

订阅聊天事件

```typescript
await invoke('subscribe_chat_events');

// 监听事件
await listen('chat-event', (event) => {
  console.log('Received:', event.payload);
});
```

### unsubscribe_chat_events

取消订阅聊天事件

```typescript
await invoke('unsubscribe_chat_events');
```

## 迁移指南

### 从 useAiChat 迁移

```typescript
// ❌ 旧代码
import { useAiChat } from '@hooks/useAiChat';

const chat = useAiChat({
  threadId,
  apiUrl: 'http://localhost:38080',
  authToken,
});

// ✅ 新代码
import { useAiChatTauri } from '@hooks/useAiChatTauri';

const chat = useAiChatTauri({
  threadId,
});
```

### API 对比

| 功能 | 旧 API (HTTP) | 新 API (Tauri IPC) |
|------|---------------|-------------------|
| 发送消息 | `fetch('/api/chat/send')` | `invoke('send_chat_message')` |
| 接收事件 | `EventSource('/api/chat/events')` | `listen('chat-event')` |
| 连接管理 | 手动管理 | 自动管理 |
| 类型安全 | ❌ | ✅ |
| 性能 | 较慢 | 快速 |

## 常见问题

### Q: 如何处理连接断开?

A: Hook 会自动重连,你只需要监听 `isConnected` 状态:

```typescript
{chat.isConnected ? (
  <span>✅ Connected</span>
) : (
  <span>⚠️  Reconnecting...</span>
)}
```

### Q: 如何处理错误?

A: 使用 `onError` 回调或检查 `error` 状态:

```typescript
const chat = useAiChatTauri({
  threadId,
  onError: (error) => {
    toast.error(error);
  },
});

// 或者
{chat.error && <div>❌ {chat.error}</div>}
```

### Q: 如何显示思考状态?

A: 检查 `thinkingMessage` 状态:

```typescript
{chat.thinkingMessage && (
  <div>💭 {chat.thinkingMessage}</div>
)}
```

### Q: 如何清空消息历史?

A: 调用 `clearMessages()`:

```typescript
<button onClick={chat.clearMessages}>
  Clear History
</button>
```

## 调试技巧

### 1. 查看事件日志

```typescript
await listen('chat-event', (event) => {
  console.log('📨 Event:', event.payload);
});
```

### 2. 检查连接状态

```typescript
console.log('Connected:', chat.isConnected);
```

### 3. 查看 Rust 日志

```bash
# 启动应用时查看日志
cargo tauri dev
```

## 性能优化

### 1. 避免频繁重新订阅

```typescript
// ✅ 正确 - 只在 threadId 变化时重新订阅
useEffect(() => {
  // 订阅逻辑
}, [threadId]);

// ❌ 错误 - 每次渲染都重新订阅
useEffect(() => {
  // 订阅逻辑
});
```

### 2. 使用 useCallback 优化回调

```typescript
const handleError = useCallback((error: string) => {
  toast.error(error);
}, []);

const chat = useAiChatTauri({
  threadId,
  onError: handleError,
});
```

## 相关文档

- `TAURI_IPC_MIGRATION_COMPLETE.md` - 完整迁移报告
- `ARCHITECTURE_EVOLUTION.md` - 架构演进方案
- `MIGRATE_TO_TAURI_IPC.md` - 详细迁移指南
