# Desktop Client 架构总览

> 最后更新: 2025-01-XX

## 系统架构

Desktop Client 是一个基于 Tauri 的桌面应用,采用前后端分离架构:

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
│                      │  外部服务     │   │
│                      │  (IronClaw)  │   │
│                      └──────────────┘   │
└─────────────────────────────────────────┘
```

## 核心组件

### 前端 (React + TypeScript)

**技术栈**:
- React 18
- TypeScript 5
- Vite 5
- TailwindCSS

**主要功能**:
- 用户界面渲染
- 用户交互处理
- 状态管理
- 通过 Tauri IPC 与后端通信

**目录结构**:
```
ui/
├── src/
│   ├── app/
│   │   ├── components/  # UI 组件
│   │   ├── hooks/       # React Hooks
│   │   ├── pages/       # 页面组件
│   │   └── utils/       # 工具函数
│   └── main.tsx         # 入口文件
```

### 后端 (Rust + Tauri)

**技术栈**:
- Rust 1.92+
- Tauri 2.0
- Tokio (异步运行时)
- libSQL (本地数据库)

**主要功能**:
- Tauri 命令处理
- 本地数据存储
- 认证和会话管理
- DLP 扫描
- 与 IronClaw 服务通信

**目录结构**:
```
src/
├── main.rs              # 应用入口
├── commands.rs          # Tauri 命令 (1728 行)
├── auth.rs              # 认证模块
├── dlp_integration.rs   # DLP 集成
├── embedded_server.rs   # 服务器健康检查
└── platform_utils.rs    # 平台工具
```

## 通信机制

### Tauri IPC

所有前后端通信都通过 Tauri IPC 进行:

```typescript
// 前端调用后端命令
import { invoke } from '@tauri-apps/api/core';

const result = await invoke('send_chat_message', {
  threadId: 'thread-123',
  content: 'Hello!',
});
```

```rust
// 后端命令定义
#[tauri::command]
pub async fn send_chat_message(
    state: State<'_, CommandState>,
    thread_id: String,
    content: String,
) -> Result<SendMessageResponse, String> {
    // 实现
}
```

### 事件系统

后端可以主动向前端发送事件:

```rust
// 后端发送事件
window.emit("chat-event", ChatEvent {
    type_: "response".to_string(),
    content: "Hello!".to_string(),
})?;
```

```typescript
// 前端监听事件
import { listen } from '@tauri-apps/api/event';

const unlisten = await listen('chat-event', (event) => {
  console.log('Received:', event.payload);
});
```

## 数据流

### 聊天消息流

```
用户输入
  ↓
前端 (ChatTabTauri)
  ↓
invoke('send_chat_message')
  ↓
Rust 后端 (commands.rs)
  ↓
HTTP POST → IronClaw Server
  ↓
SSE 订阅 (后台任务)
  ↓
window.emit('chat-event')
  ↓
前端监听事件
  ↓
更新 UI
```

### DLP 扫描流

```
用户输入
  ↓
前端 (useDlpScan)
  ↓
invoke('scan_user_input')
  ↓
Rust 后端 (dlp_integration.rs)
  ↓
ironclaw_safety crate
  ↓
返回扫描结果
  ↓
前端显示脱敏内容
```

## 安全机制

### 认证

- 主密码加密 (Argon2)
- 会话令牌管理
- 自动令牌刷新
- 安全存储 (OS Keychain)

### DLP (数据防泄漏)

- 实时输入扫描
- 敏感信息脱敏
- 规则引擎
- 审计日志

### 加密存储

- libSQL 加密数据库
- AES-GCM 加密
- 密钥派生 (HKDF)

## 性能优化

### 前端

- 代码分割 (Vite)
- 懒加载组件
- 虚拟滚动 (长列表)
- 防抖和节流

### 后端

- 异步 I/O (Tokio)
- 连接池
- 缓存机制
- 批量操作

## 部署架构

### 开发环境

```
Desktop Client (开发模式)
  ↓
连接到 → IronClaw Server (端口 38080)
```

启动命令:
```bash
# 1. 启动 IronClaw 服务器
cargo run -- run --no-onboard

# 2. 启动 Desktop Client
cd desktop-client
cargo tauri dev
```

### 生产环境

```
Desktop Client (独立应用)
  ↓
连接到 → IronClaw Server (配置的端口)
```

打包命令:
```bash
cd desktop-client
cargo tauri build
```

## 技术决策

### 为什么选择 Tauri?

1. **跨平台**: 一次编写,多平台运行
2. **性能**: Rust 后端,性能优异
3. **安全**: 沙箱隔离,权限控制
4. **体积小**: 比 Electron 小 10 倍
5. **原生体验**: 使用系统 WebView

### 为什么使用 Tauri IPC?

1. **类型安全**: TypeScript + Rust 类型检查
2. **性能**: 进程内通信,无网络开销
3. **安全**: 无需暴露 HTTP 端口
4. **简单**: 统一的通信机制
5. **可靠**: 自动序列化和错误处理

### 为什么连接外部 IronClaw 服务?

1. **职责分离**: Desktop Client 专注于 UI
2. **复用能力**: 复用 IronClaw 核心功能
3. **灵活部署**: 可以连接不同的服务器
4. **易于维护**: 独立更新和部署

## 未来规划

### 短期 (1-2 月)

- [ ] 重构 commands 模块 (拆分为子模块)
- [ ] 优化测试覆盖率
- [ ] 完善文档

### 中期 (3-6 月)

- [ ] 支持多账户
- [ ] 离线模式
- [ ] 插件系统

### 长期 (6-12 月)

- [ ] 完全独立 (内嵌 IronClaw 核心)
- [ ] 移动端支持
- [ ] 云同步

## 相关文档

- [架构演进](evolution.md) - 架构变更历史
- [Tauri IPC 设计](tauri-ipc.md) - IPC 通信机制
- [快速开始](../guides/quick-start.md) - 快速上手指南

---

**注意**: 本文档描述的是当前架构状态。架构会随着项目发展持续演进,请参考 [架构演进](evolution.md) 了解历史变更。
