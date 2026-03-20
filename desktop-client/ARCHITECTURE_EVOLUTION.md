# Desktop Client 架构演进方案

## 当前架构问题

### 问题 1: 混合的通信方式

当前 Desktop Client 使用了两种通信方式:

1. **Tauri IPC 命令** (推荐方式)
   - 用于: DLP 扫描、配置管理、认证等
   - 优点: 类型安全、不需要网络、性能好
   - 示例: `invoke('scan_user_input', { content })`

2. **HTTP API 直连** (不推荐方式)
   - 用于: 聊天消息发送、SSE 事件接收
   - 缺点: 依赖外部服务、需要管理端口、不够安全
   - 示例: `fetch('http://localhost:38080/api/chat/send')`

### 问题 2: 依赖外部 IronClaw 服务

当前架构要求:
- 必须先启动 IronClaw 服务器 (端口 38080)
- Desktop Client 通过 HTTP 调用 IronClaw API
- 如果 IronClaw 服务不可用,聊天功能无法使用

这种设计的问题:
1. **部署复杂**: 用户需要同时运行两个进程
2. **端口冲突**: 可能与其他服务冲突
3. **安全风险**: HTTP 端口暴露在本地网络
4. **不够独立**: Desktop Client 不能独立运行

## 目标架构

### 理想状态: 完全独立的 Desktop Client

```
┌─────────────────────────────────────────┐
│         Desktop Client (Tauri)          │
│                                         │
│  ┌─────────────┐      ┌──────────────┐ │
│  │   前端 UI    │ IPC  │  Rust 后端   │ │
│  │  (React)    │◄────►│  (Commands)  │ │
│  └─────────────┘      └──────────────┘ │
│                             │           │
│                             ▼           │
│                      ┌──────────────┐   │
│                      │  核心功能库   │   │
│                      │  (ironclaw)  │   │
│                      └──────────────┘   │
└─────────────────────────────────────────┘
```

**优点**:
- 单一进程,部署简单
- 不需要网络端口
- 更安全,所有通信通过 IPC
- 完全独立,不依赖外部服务

## 演进路线图

### 阶段 1: 当前状态 (已完成)

**架构**:
```
Desktop Client ──HTTP──► IronClaw Server (38080)
     │
     └──IPC──► Tauri Commands (DLP, Auth, etc.)
```

**问题**:
- 聊天功能依赖 HTTP API
- 需要外部 IronClaw 服务

### 阶段 2: 混合模式 (过渡方案)

**目标**: 将聊天功能迁移到 Tauri 命令

**实现步骤**:

1. **创建聊天相关的 Tauri 命令**
   ```rust
   // desktop-client/src/commands.rs
   
   #[tauri::command]
   pub async fn send_chat_message(
       state: tauri::State<'_, CommandState>,
       thread_id: String,
       content: String,
   ) -> Result<SendMessageResponse> {
       // 调用 ironclaw 核心功能
       state.api_client.send_message(SendMessageRequest {
           thread_id,
           content,
       }).await
   }
   
   #[tauri::command]
   pub async fn subscribe_chat_events(
       state: tauri::State<'_, CommandState>,
       window: tauri::Window,
   ) -> Result<()> {
       // 使用 Tauri 事件系统替代 SSE
       let api_client = state.api_client.clone();
       
       tokio::spawn(async move {
           let mut events = api_client.subscribe_events().await;
           
           while let Some(event) = events.next().await {
               // 发送事件到前端
               window.emit("chat-event", event).ok();
           }
       });
       
       Ok(())
   }
   ```

2. **前端使用 Tauri 事件系统**
   ```typescript
   // desktop-client/src-ui/src/app/hooks/useAiChat.ts
   
   import { invoke } from '@tauri-apps/api/core';
   import { listen } from '@tauri-apps/api/event';
   
   export function useAiChat(options: UseAiChatOptions) {
     // 订阅聊天事件
     useEffect(() => {
       const unlisten = listen('chat-event', (event) => {
         handleChatEvent(event.payload);
       });
       
       // 启动事件订阅
       invoke('subscribe_chat_events');
       
       return () => {
         unlisten.then(fn => fn());
       };
     }, []);
     
     // 发送消息
     const sendMessage = async (content: string) => {
       const result = await invoke('send_chat_message', {
         threadId,
         content,
       });
       return result;
     };
   }
   ```

**优点**:
- 不再依赖 HTTP API
- 所有通信通过 IPC
- 更安全,更快速

**缺点**:
- 仍然依赖外部 IronClaw 服务 (通过 api_client)

### 阶段 3: 完全独立 (最终目标)

**目标**: Desktop Client 完全独立,不依赖外部服务

**实现步骤**:

1. **将 IronClaw 核心功能作为库依赖**
   ```toml
   # desktop-client/Cargo.toml
   
   [dependencies]
   ironclaw = { path = "../ironclaw", default-features = false, features = ["core"] }
   ```

2. **直接调用核心功能**
   ```rust
   // desktop-client/src/commands.rs
   
   use ironclaw::agent::Agent;
   use ironclaw::db::Database;
   
   pub struct CommandState {
       pub agent: Arc<Mutex<Agent>>,
       pub db: Arc<Database>,
       // ... 其他状态
   }
   
   #[tauri::command]
   pub async fn send_chat_message(
       state: tauri::State<'_, CommandState>,
       thread_id: String,
       content: String,
   ) -> Result<SendMessageResponse> {
       // 直接调用 Agent
       let agent = state.agent.lock().await;
       agent.send_message(thread_id, content).await
   }
   ```

3. **使用 Tauri 事件系统替代 SSE**
   ```rust
   #[tauri::command]
   pub async fn start_agent_loop(
       state: tauri::State<'_, CommandState>,
       window: tauri::Window,
   ) -> Result<()> {
       let agent = state.agent.clone();
       
       tokio::spawn(async move {
           let mut events = agent.subscribe_events().await;
           
           while let Some(event) = events.next().await {
               window.emit("agent-event", event).ok();
           }
       });
       
       Ok(())
   }
   ```

**优点**:
- 完全独立,不依赖外部服务
- 单一进程,部署简单
- 更安全,所有通信通过 IPC
- 更快速,无网络开销

**挑战**:
- 需要重构 IronClaw 核心功能为库
- 需要处理数据库和文件系统访问
- 需要管理 Agent 生命周期

## 实施建议

### 短期 (1-2 周)

**优先级 P0**: 修复当前问题
- ✅ 修复 API 端口配置
- ✅ 修复 DLP 配置频繁调用
- ✅ 统一 API 配置管理

### 中期 (1-2 月)

**优先级 P1**: 迁移到 Tauri 命令
1. 创建聊天相关的 Tauri 命令
2. 使用 Tauri 事件系统替代 SSE
3. 移除 HTTP API 依赖
4. 更新文档和测试

**检查清单**:
- [ ] 创建 `send_chat_message` 命令
- [ ] 创建 `subscribe_chat_events` 命令
- [ ] 前端使用 Tauri 事件系统
- [ ] 移除 HTTP API 调用
- [ ] 更新测试
- [ ] 更新文档

### 长期 (3-6 月)

**优先级 P2**: 完全独立
1. 重构 IronClaw 核心功能为库
2. Desktop Client 直接依赖核心库
3. 移除外部服务依赖
4. 优化性能和安全性

**检查清单**:
- [ ] 提取 IronClaw 核心功能为库
- [ ] Desktop Client 依赖核心库
- [ ] 移除 api_client
- [ ] 移除 embedded_server
- [ ] 更新所有测试
- [ ] 更新文档

## 技术细节

### Tauri 事件系统 vs SSE

| 特性 | SSE | Tauri 事件 |
|------|-----|-----------|
| 通信方式 | HTTP | IPC |
| 性能 | 较慢 (网络开销) | 快速 (进程内) |
| 安全性 | 需要端口 | 无端口 |
| 类型安全 | 需要手动解析 | 自动序列化 |
| 调试 | 需要网络工具 | Tauri DevTools |
| 部署 | 需要服务器 | 无需服务器 |

### 代码示例对比

**当前方式 (HTTP + SSE)**:
```typescript
// ❌ 不推荐
const response = await fetch('http://localhost:38080/api/chat/send', {
  method: 'POST',
  body: JSON.stringify({ content }),
});

const eventSource = new EventSource('http://localhost:38080/api/chat/events');
eventSource.addEventListener('response', (e) => {
  handleResponse(JSON.parse(e.data));
});
```

**推荐方式 (Tauri IPC)**:
```typescript
// ✅ 推荐
const response = await invoke('send_chat_message', { content });

const unlisten = await listen('chat-event', (event) => {
  handleResponse(event.payload);
});
```

## 参考资料

- [Tauri 事件系统文档](https://tauri.app/v1/guides/features/events/)
- [Tauri IPC 最佳实践](https://tauri.app/v1/guides/features/command/)
- `desktop-client/src/commands.rs` - 现有命令实现
- `desktop-client/DESKTOP_CLIENT_FEATURE_CHECKLIST.md` - 功能检查清单

## 总结

**当前问题**: Desktop Client 混合使用 HTTP API 和 Tauri 命令,依赖外部 IronClaw 服务。

**解决方案**: 分阶段迁移到完全基于 Tauri IPC 的架构,最终实现完全独立的 Desktop Client。

**下一步**: 创建聊天相关的 Tauri 命令,使用 Tauri 事件系统替代 SSE。
