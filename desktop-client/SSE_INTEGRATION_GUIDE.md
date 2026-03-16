# SSE 集成指南

## 概述

本指南说明如何在 ChatTab 组件中集成 SSE 客户端，以支持实时消息流。

## 已完成的工作

### 后端
- ✅ `chat_events_handler` 已实现
- ✅ SSE 事件流支持
- ✅ 历史消息回放

### Rust 端
- ✅ `SseClient` 实现
- ✅ 事件类型定义
- ✅ 20 个 SSE 集成测试

### TypeScript 端
- ✅ `SseClient` 实现
- ✅ 事件处理器支持
- ✅ Fetch API 集成

## 集成步骤

### 1. 在 ChatTab 中导入 SSE 客户端

```typescript
import { createSseClient, type SseEvent } from '../../utils/sse';
```

### 2. 添加 SSE 状态管理

```typescript
const [sseClient, setSseClient] = useState<SseClient | null>(null);
const [sseConnected, setSseConnected] = useState(false);
```

### 3. 初始化 SSE 连接

```typescript
useEffect(() => {
  if (selectedConversation) {
    const client = createSseClient(
      'http://localhost:3000',
      'your-auth-token'
    );
    
    // 注册事件处理器
    client.onEvent((event: SseEvent) => {
      if (event.type === 'message') {
        // 处理新消息
        setMessages(prev => [...prev, {
          id: event.data.message_id,
          thread_id: event.data.thread_id,
          role: event.data.role,
          content: event.data.content,
          created_at: new Date().toISOString(),
        }]);
      } else if (event.type === 'thread_state') {
        // 处理对话状态变化
        console.log('Thread state changed:', event.data.state);
      }
    });
    
    // 连接到 SSE 流
    client.connect()
      .then(() => {
        setSseConnected(true);
        setSseClient(client);
      })
      .catch(err => {
        console.error('Failed to connect to SSE:', err);
        setSseConnected(false);
      });
    
    return () => {
      client.disconnect();
      setSseConnected(false);
    };
  }
}, [selectedConversation]);
```

### 4. 替换轮询方式

当前的 `handleSend` 函数使用轮询方式获取消息。使用 SSE 后，可以简化为：

```typescript
const handleSend = async () => {
  if (!inputText.trim() || !selectedConversation) return;
  
  const content = inputText.trim();
  setInputText('');
  setLoading(true);
  
  try {
    await threadApi.sendMessage(selectedConversation, content);
    // SSE 会自动接收新消息，无需轮询
  } catch (err) {
    console.error('Failed to send message:', err);
  } finally {
    setLoading(false);
  }
};
```

### 5. 添加 SSE 连接状态指示

```typescript
<div className={`p-2 ${sseConnected ? 'bg-green-100' : 'bg-red-100'}`}>
  SSE: {sseConnected ? '已连接' : '未连接'}
</div>
```

## 事件类型

### Message 事件
```typescript
{
  type: 'message',
  data: {
    thread_id: string,
    message_id: string,
    content: string,
    role: string
  }
}
```

### ThreadState 事件
```typescript
{
  type: 'thread_state',
  data: {
    thread_id: string,
    state: string
  }
}
```

### AuthCompleted 事件
```typescript
{
  type: 'auth_completed',
  data: {
    extension_name: string,
    success: boolean
  }
}
```

### AuthRequired 事件
```typescript
{
  type: 'auth_required',
  data: {
    extension_name: string,
    instructions?: string
  }
}
```

## 错误处理

### 连接失败

```typescript
client.connect()
  .catch(err => {
    console.error('SSE connection failed:', err);
    // 回退到轮询方式
    setUsePolling(true);
  });
```

### 事件处理错误

```typescript
client.onEvent((event: SseEvent) => {
  try {
    // 处理事件
  } catch (err) {
    console.error('Failed to handle SSE event:', err);
  }
});
```

## 性能优化

### 1. 事件去重

```typescript
const processedMessageIds = new Set<string>();

client.onEvent((event: SseEvent) => {
  if (event.type === 'message') {
    if (!processedMessageIds.has(event.data.message_id)) {
      processedMessageIds.add(event.data.message_id);
      // 处理消息
    }
  }
});
```

### 2. 批量更新

```typescript
let pendingMessages: Message[] = [];
let updateTimer: NodeJS.Timeout | null = null;

client.onEvent((event: SseEvent) => {
  if (event.type === 'message') {
    pendingMessages.push({...});
    
    if (!updateTimer) {
      updateTimer = setTimeout(() => {
        setMessages(prev => [...prev, ...pendingMessages]);
        pendingMessages = [];
        updateTimer = null;
      }, 100);
    }
  }
});
```

### 3. 内存管理

```typescript
const MAX_MESSAGES = 1000;

client.onEvent((event: SseEvent) => {
  if (event.type === 'message') {
    setMessages(prev => {
      const updated = [...prev, newMessage];
      if (updated.length > MAX_MESSAGES) {
        return updated.slice(-MAX_MESSAGES);
      }
      return updated;
    });
  }
});
```

## 测试

### 单元测试

```typescript
import { createSseClient } from '../utils/sse';

describe('SseClient', () => {
  it('should create client', () => {
    const client = createSseClient('http://localhost:3000', 'token');
    expect(client).toBeDefined();
  });

  it('should register event handler', () => {
    const client = createSseClient('http://localhost:3000', 'token');
    const handler = jest.fn();
    client.onEvent(handler);
    // 验证处理器已注册
  });
});
```

### 集成测试

```typescript
it('should receive messages via SSE', async () => {
  const client = createSseClient('http://localhost:3000', 'token');
  
  const messageReceived = new Promise(resolve => {
    client.onEvent((event) => {
      if (event.type === 'message') {
        resolve(event.data);
      }
    });
  });
  
  await client.connect();
  // 发送消息
  const message = await messageReceived;
  expect(message.content).toBe('Hello');
});
```

## 故障排查

### SSE 连接失败

1. 检查后端是否运行在 `http://localhost:3000`
2. 检查认证令牌是否正确
3. 检查浏览器控制台是否有 CORS 错误

### 消息未接收

1. 检查 SSE 连接是否已建立
2. 检查事件处理器是否已注册
3. 检查后端是否发送事件

### 性能问题

1. 检查消息数量是否过多
2. 实现消息去重和批量更新
3. 限制消息历史大小

## 后续工作

1. ✅ 实现 SSE 客户端（Rust 和 TypeScript）
2. ⏳ 在 ChatTab 中集成 SSE 客户端
3. ⏳ 替换轮询方式为实时 SSE 流
4. ⏳ 添加 SSE 连接状态指示
5. ⏳ 实现 SSE 重连机制
6. ⏳ 添加 SSE 性能监控

## 参考资源

- [MDN: Server-Sent Events](https://developer.mozilla.org/en-US/docs/Web/API/Server-sent_events)
- [Fetch API](https://developer.mozilla.org/en-US/docs/Web/API/Fetch_API)
- [EventSource API](https://developer.mozilla.org/en-US/docs/Web/API/EventSource)
