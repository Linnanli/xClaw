# 简化的聊天架构方案

## 当前架构的问题

### 复杂度过高
```
前端 → Tauri Command → API Client → 后端 API
                ↓
        SSE 订阅管理器
                ↓
        后台任务轮询 SSE
                ↓
        解析事件 → Emit 到前端
                ↓
        前端监听事件
```

**问题**：
1. 需要管理订阅状态
2. 需要处理重连逻辑
3. 需要同步前后端订阅状态
4. 容易出现内存泄漏
5. 调试困难

## 推荐方案：简化架构

### 方案 1: 直接使用 Tauri HTTP 客户端（推荐）

**架构**：
```
前端 → Tauri HTTP Plugin → 后端 API (直接)
```

**优点**：
- 最简单，无需中间层
- 使用 Tauri 的 `@tauri-apps/plugin-http`
- 自动处理 CORS
- 无需管理订阅状态

**实现**：

#### 1. 安装依赖
```bash
cd desktop-client
npm install @tauri-apps/plugin-http
```

#### 2. 配置 Cargo.toml
```toml
[dependencies]
tauri-plugin-http = "2.0"
```

#### 3. 注册插件
```rust
// src/main.rs
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_http::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

#### 4. 前端直接调用
```typescript
import { fetch } from '@tauri-apps/plugin-http';

export function useAiChatSimple(options: UseAiChatOptions) {
  const [messages, setMessages] = useState<Message[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const sendMessage = async (content: string) => {
    setIsLoading(true);

    // 1. 添加用户消息
    const userMessage = { role: 'user', content };
    setMessages(prev => [...prev, userMessage]);

    try {
      // 2. 直接调用后端 API
      const response = await fetch('http://localhost:3000/api/chat/send', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${token}`,
        },
        body: JSON.stringify({
          thread_id: threadId,
          content,
        }),
      });

      const data = await response.json();

      // 3. 添加 AI 响应
      setMessages(prev => [...prev, {
        role: 'assistant',
        content: data.content,
      }]);
    } catch (error) {
      console.error('Failed to send message:', error);
    } finally {
      setIsLoading(false);
    }
  };

  return { messages, sendMessage, isLoading };
}
```

**优点**：
- 代码量减少 80%
- 无需管理订阅
- 无需处理事件监听器
- 无需担心内存泄漏
- 调试简单

**缺点**：
- 无法实时显示"思考中"状态
- 需要等待完整响应

---

### 方案 2: 使用轮询（如果需要实时状态）

**架构**：
```
前端 → 定时轮询 → Tauri HTTP → 后端 API
```

**实现**：
```typescript
export function useAiChatPolling(options: UseAiChatOptions) {
  const [messages, setMessages] = useState<Message[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const pollingRef = useRef<NodeJS.Timeout | null>(null);

  // 轮询获取新消息
  useEffect(() => {
    const pollMessages = async () => {
      try {
        const response = await fetch(
          `http://localhost:3000/api/chat/messages?thread_id=${threadId}&since=${lastMessageId}`
        );
        const newMessages = await response.json();
        
        if (newMessages.length > 0) {
          setMessages(prev => [...prev, ...newMessages]);
        }
      } catch (error) {
        console.error('Failed to poll messages:', error);
      }
    };

    // 每 1 秒轮询一次
    pollingRef.current = setInterval(pollMessages, 1000);

    return () => {
      if (pollingRef.current) {
        clearInterval(pollingRef.current);
      }
    };
  }, [threadId, lastMessageId]);

  const sendMessage = async (content: string) => {
    // 同方案 1
  };

  return { messages, sendMessage, isLoading };
}
```

**优点**：
- 仍然很简单
- 可以获取实时更新
- 无需管理 SSE 连接

**缺点**：
- 有轮询延迟（1秒）
- 增加服务器负载

---

### 方案 3: 使用 WebSocket（如果需要真正的实时通信）

**架构**：
```
前端 → Tauri WebSocket → 后端 WebSocket
```

**实现**：
```typescript
import { WebSocket } from '@tauri-apps/plugin-websocket';

export function useAiChatWebSocket(options: UseAiChatOptions) {
  const [messages, setMessages] = useState<Message[]>([]);
  const wsRef = useRef<WebSocket | null>(null);

  useEffect(() => {
    // 连接 WebSocket
    const ws = new WebSocket('ws://localhost:3000/api/chat/ws');

    ws.onmessage = (event) => {
      const message = JSON.parse(event.data);
      setMessages(prev => [...prev, message]);
    };

    wsRef.current = ws;

    return () => {
      ws.close();
    };
  }, []);

  const sendMessage = async (content: string) => {
    if (wsRef.current) {
      wsRef.current.send(JSON.stringify({
        type: 'message',
        content,
      }));
    }
  };

  return { messages, sendMessage };
}
```

**优点**：
- 真正的实时通信
- 双向通信
- 比 SSE 更简单

**缺点**：
- 需要后端支持 WebSocket
- 需要处理重连

---

## 推荐选择

### 如果不需要实时"思考中"状态
→ **方案 1: 直接 HTTP 调用**（最简单）

### 如果需要实时状态但可以接受 1 秒延迟
→ **方案 2: 轮询**（简单且实用）

### 如果需要真正的实时通信
→ **方案 3: WebSocket**（需要后端改造）

---

## 迁移步骤（推荐方案 1）

### 1. 安装依赖
```bash
cd desktop-client
npm install @tauri-apps/plugin-http
```

### 2. 更新 Cargo.toml
```toml
[dependencies]
tauri-plugin-http = "2.0"
```

### 3. 注册插件
```rust
// src/main.rs
.plugin(tauri_plugin_http::init())
```

### 4. 创建新的 Hook
```typescript
// src-ui/src/app/hooks/useAiChatSimple.ts
import { fetch } from '@tauri-apps/plugin-http';

export function useAiChatSimple(options: UseAiChatOptions) {
  // 实现如上
}
```

### 5. 替换使用
```typescript
// ChatTabTauri.tsx
import { useAiChatSimple } from '@/app/hooks/useAiChatSimple';

const chat = useAiChatSimple({ threadId });
```

### 6. 删除复杂代码
- 删除 `subscribe_chat_events` 命令
- 删除 `unsubscribe_chat_events` 命令
- 删除 `SseSubscriptionManager`
- 删除 `useAiChatTauri` 的复杂逻辑

---

## 代码量对比

### 当前方案
- Rust: ~500 行（SSE 管理、事件解析、订阅管理）
- TypeScript: ~300 行（事件监听、状态管理、清理逻辑）
- 总计: ~800 行

### 简化方案 1
- Rust: ~0 行（使用 Tauri HTTP 插件）
- TypeScript: ~50 行（简单的 HTTP 调用）
- 总计: ~50 行

**减少 94% 的代码！**

---

## 性能对比

| 方案 | 延迟 | 资源占用 | 复杂度 | 可靠性 |
|------|------|----------|--------|--------|
| 当前 SSE | 实时 | 高（后台任务） | 高 | 中（容易泄漏） |
| 方案 1 HTTP | 响应时间 | 低 | 低 | 高 |
| 方案 2 轮询 | 1秒 | 中 | 低 | 高 |
| 方案 3 WebSocket | 实时 | 中 | 中 | 高 |

---

## 结论

**强烈推荐使用方案 1（直接 HTTP 调用）**，因为：

1. **极简** - 代码量减少 94%
2. **可靠** - 无内存泄漏风险
3. **易维护** - 逻辑清晰
4. **易调试** - 标准 HTTP 请求
5. **足够用** - 对于聊天应用，等待完整响应是可接受的

如果确实需要实时"思考中"状态，可以考虑方案 2（轮询），仍然比当前的 SSE 方案简单得多。
