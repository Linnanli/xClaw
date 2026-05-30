# IronClaw AI 对话流架构文档

> 本文档描述 AI 对话从用户输入到 LLM 响应的完整数据流，覆盖 Desktop Client（前端 + Tauri 后端）、Admin Backend（API + 前端）、IronClaw 核心引擎三个模块。

---

## 1. 系统总览

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Desktop Client (Tauri)                       │
│  ┌──────────────────┐    Tauri IPC     ┌──────────────────────────┐ │
│  │  React 前端       │ ◄─── invoke ──► │  Rust 后端 (ipc/*.rs)    │ │
│  │  @assistant-ui    │ ◄── emit ──────►│  TauriChannel            │ │
│  │  + DLP 拦截层     │                 │  + 内嵌 IronClaw Engine  │ │
│  └────────┬─────────┘                 └───────────┬──────────────┘ │
│           │ HTTP                                   │                │
│           ▼                                        ▼                │
│  ┌──────────────────┐                 ┌──────────────────────────┐ │
│  │  Admin Backend    │                 │  IronClaw Agent          │ │
│  │  /api/chat/       │                 │  (消息循环 + LLM 推理)   │ │
│  │  completions      │                 │                          │ │
│  └──────────────────┘                 └──────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────┐
│                     Admin Backend (独立服务)                         │
│  ┌──────────────────┐    HTTP/REST    ┌──────────────────────────┐ │
│  │  React 前端       │ ◄────────────► │  Rust 后端 (Axum)        │ │
│  │  (管理控制台)     │                │  + Chat Proxy            │ │
│  │  端口 5174        │                │  + DLP/RBAC 管理         │ │
│  └──────────────────┘                │  端口 3000               │ │
│                                       └──────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 2. 对话流数据路径（两条并行路径）

Desktop Client 存在两条独立的对话路径：

### 路径 A：@assistant-ui → Admin Backend Chat Proxy（当前活跃路径）

```
用户输入
  │
  ▼
┌─────────────────────────────────────────────────────────┐
│ 前端 DLP 拦截层 (ComposerAction / useDlpScan hook)     │
│  invoke('scan_user_input', { content })                 │
│  → Tauri Rust DLP 引擎扫描                              │
│  → 返回 Clean / PiiDetected(脱敏内容) / SecretBlocked   │
│  → SecretBlocked: 阻止发送，显示 DlpBlockedDialog       │
│  → PiiDetected: 替换为脱敏内容，显示 DlpWarningBanner   │
│  → Clean: 放行                                          │
└──────────────────────────┬──────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────┐
│ ChatRuntimeProvider (@assistant-ui/react-ai-sdk)        │
│                                                         │
│  工具: useChatRuntime + AssistantChatTransport           │
│  协议: Vercel AI SDK Data Stream                        │
│                                                         │
│  POST http://localhost:3000/api/chat/completions        │
│  Content-Type: application/json                         │
│  Body: {                                                │
│    "model": "deepseek-chat",                            │
│    "messages": [                                        │
│      {"role": "user", "content": "..."},                │
│      {"role": "assistant", "content": "..."}            │
│    ],                                                   │
│    "stream": true                                       │
│  }                                                      │
└──────────────────────────┬──────────────────────────────┘
                           │ HTTP POST
                           ▼
┌─────────────────────────────────────────────────────────┐
│ Admin Backend: chat_completions_handler                  │
│ (admin-backend/src/chat_proxy.rs)                       │
│                                                         │
│  1. 反序列化请求（兼容 content string / parts array）   │
│  2. 查询 model_configs 表获取 API 配置                  │
│     SELECT api_base_url, api_key, provider              │
│     FROM model_configs WHERE model_id = $1              │
│  3. 根据 provider 选择协议:                             │
│     - "anthropic" → ProviderProtocol::Anthropic         │
│     - "ollama"    → ProviderProtocol::Ollama            │
│     - 其他        → ProviderProtocol::OpenAiCompletions │
│  4. 通过 ironclaw_llm crate 创建 LLM provider          │
│  5. 调用 llm.complete(request)                          │
│  6. 编码为 Vercel AI SDK Data Stream 格式返回           │
└──────────────────────────┬──────────────────────────────┘
                           │ HTTP Response
                           ▼
┌─────────────────────────────────────────────────────────┐
│ 响应格式: Vercel AI SDK Data Stream (NDJSON)            │
│                                                         │
│  {"type":"text-start","id":"msg-uuid"}                  │
│  {"type":"text-delta","id":"msg-uuid","delta":"内容"}   │
│  {"type":"text-end","id":"msg-uuid"}                    │
│                                                         │
│  前端 @assistant-ui 自动解析并渲染到 Thread 组件        │
└─────────────────────────────────────────────────────────┘
```

### 路径 B：Tauri IPC → 内嵌 IronClaw Engine（Agent 完整能力）

```
用户输入
  │
  ▼
┌─────────────────────────────────────────────────────────┐
│ 前端: invoke('send_chat_message', {                     │
│   thread_id: "uuid",                                    │
│   content: "用户消息",                                   │
│   attachments: [...]                                    │
│ })                                                      │
└──────────────────────────┬──────────────────────────────┘
                           │ Tauri IPC (进程内调用)
                           ▼
┌─────────────────────────────────────────────────────────┐
│ ipc/chat.rs::send_chat_message()                        │
│                                                         │
│  1. 从 Tauri State 获取 AppState                        │
│  2. 构建 IncomingMessage { content, thread_id, ... }    │
│  3. 通过 TauriChannel.sender().send(msg) 注入消息       │
└──────────────────────────┬──────────────────────────────┘
                           │ mpsc channel
                           ▼
┌─────────────────────────────────────────────────────────┐
│ IronClaw Agent 消息循环                                  │
│ (ironclaw/src/agent/)                                   │
│                                                         │
│  1. 意图路由 (Intent Router)                            │
│  2. LLM 推理 (ironclaw_llm crate)                      │
│  3. 工具调用 (Tools Registry: MCP/WASM/内置)            │
│  4. 安全检查 (Safety: 提示注入防御 + DLP)               │
│  5. 记忆检索 (Workspace: 全文 + 向量混合搜索)           │
│  6. 生成响应                                            │
└──────────────────────────┬──────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────┐
│ TauriChannel.respond() / send_status()                  │
│                                                         │
│  app_handle.emit("chat-event", ChatEvent { ... })       │
│                                                         │
│  ChatEvent 类型 (serde tag = "type"):                   │
│  ┌─────────────────┬──────────────────────────────────┐ │
│  │ response        │ AI 回复 {message_id, content}    │ │
│  │ thinking        │ 思考中 {message}                 │ │
│  │ status          │ 状态更新 {message, level}        │ │
│  │ error           │ 错误 {message, code}             │ │
│  │ stream_chunk    │ 流式片段 {content}               │ │
│  │ tool_started    │ 工具开始 {name}                  │ │
│  │ tool_completed  │ 工具完成 {name, success, error}  │ │
│  │ approval_needed │ 需审批 {request_id, tool_name}   │ │
│  │ image_generated │ 图片 {data_url, path}            │ │
│  │ suggestions     │ 建议 {suggestions: [...]}        │ │
│  │ connection_status│ 连接状态 {connected, message}   │ │
│  └─────────────────┴──────────────────────────────────┘ │
│                                                         │
│  前端 listen("chat-event") 接收并更新 UI                │
└─────────────────────────────────────────────────────────┘
```

---

## 3. 各模块详细说明

### 3.1 Desktop Client 前端 (React + TypeScript)

| 项目 | 说明 |
|------|------|
| 位置 | `desktop-client/ui/src/app/` |
| 框架 | React 18 + TypeScript + Vite |
| AI 对话库 | `@assistant-ui/react` + `@assistant-ui/react-ai-sdk` |
| 构建工具 | Vite (Tauri 集成) |

#### 核心组件

| 组件 | 文件 | 职责 |
|------|------|------|
| ChatRuntimeProvider | `app/runtime/ChatRuntimeProvider.tsx` | 初始化 @assistant-ui 运行时，配置 AssistantChatTransport 指向 Admin Backend |
| DLP Context | 同上 | 管理 DLP 拦截状态（blocked/redacted），通过 React Context 下发 |
| ChatInput | `app/components/ai/ChatInput.tsx` | 用户输入框，集成 DLP 扫描 |
| ChatMessage | `app/components/ai/ChatMessage.tsx` | 消息渲染，支持 Markdown、代码高亮 |
| ChatMessageList | `app/components/ai/ChatMessageList.tsx` | 消息列表，含 DLP 内联警告 |
| DlpBlockedDialog | `app/components/ai/DlpBlockedDialog.tsx` | 敏感内容阻止弹窗 |
| DlpWarningBanner | `app/components/ai/DlpWarningBanner.tsx` | PII 脱敏警告横幅 |
| ModelSelector | `app/components/ai/ModelSelector.tsx` | 模型选择器（从 Admin Backend 获取可用模型） |
| CustomModelModal | `app/components/ai/CustomModelModal.tsx` | 自定义模型配置弹窗 |

#### 关键 Hooks

| Hook | 职责 |
|------|------|
| `useDlpScan` | 调用 Tauri IPC `scan_user_input` 执行 DLP 扫描 |
| `useChatRuntime` | @assistant-ui 提供，管理对话运行时 |

#### 通信方式

| 方式 | 用途 | 协议 |
|------|------|------|
| Tauri `invoke()` | 调用 Rust 命令（DLP 扫描、模型管理等） | Tauri IPC (进程内序列化) |
| Tauri `listen()` | 接收 Rust 推送事件（chat-event） | Tauri Event (进程内) |
| HTTP POST | 发送对话到 Admin Backend | Vercel AI SDK Data Stream |

---

### 3.2 Desktop Client Tauri 后端 (Rust)

| 项目 | 说明 |
|------|------|
| 位置 | `desktop-client/src/` |
| 框架 | Tauri 2.x + Tokio 异步运行时 |
| 核心依赖 | `ironclaw` (主引擎), `ironclaw_llm` (LLM 抽象) |

#### IPC 命令清单

所有命令通过 `all_tauri_commands!()` 宏注册（`src/lib.rs`），前端通过 `invoke('命令名')` 调用。

| 模块 | 命令 | 文件 | 说明 |
|------|------|------|------|
| 聊天 | `send_chat_message` | `ipc/chat.rs` | 发送消息到 Agent |
| 聊天 | `subscribe_chat_events` | `ipc/chat.rs` | 订阅聊天事件流 |
| 聊天 | `unsubscribe_chat_events` | `ipc/chat.rs` | 取消订阅 |
| 线程 | `ic_list_threads` | `ipc/threads.rs` | 列出对话线程 |
| 线程 | `ic_create_thread` | `ipc/threads.rs` | 创建新线程 |
| 线程 | `ic_get_thread_history` | `ipc/threads.rs` | 获取线程历史 |
| DLP | `scan_user_input` | `ipc/dlp.rs` | 扫描用户输入 |
| DLP | `scan_outbound_request` | `ipc/dlp.rs` | 扫描出站请求 |
| DLP | `sanitize_for_storage` | `ipc/dlp.rs` | 存储前脱敏 |
| DLP | `get_dlp_config` | `ipc/dlp.rs` | 获取 DLP 配置 |
| DLP | `update_dlp_config` | `ipc/dlp.rs` | 更新 DLP 配置 |
| DLP | `get_dlp_statistics` | `ipc/dlp.rs` | 获取 DLP 统计 |
| DLP | `sync_dlp_rules_from_admin` | `ipc/dlp.rs` | 从 Admin 同步规则 |
| 审批 | `ic_approve_tool` | `ipc/approval.rs` | 批准工具执行 |
| 审批 | `ic_deny_tool` | `ipc/approval.rs` | 拒绝工具执行 |
| 模型 | `get_available_models` | `ipc/models.rs` | 获取可用模型列表 |
| 模型 | `get_custom_models` | `ipc/models.rs` | 获取自定义模型 |
| 模型 | `create_custom_model` | `ipc/models.rs` | 创建自定义模型 |
| 模型 | `test_model_connection` | `ipc/models.rs` | 测试模型连接 |
| 记忆 | `ic_memory_list` | `ipc/memory.rs` | 列出记忆条目 |
| 记忆 | `ic_memory_read` | `ipc/memory.rs` | 读取记忆 |
| 记忆 | `ic_memory_write` | `ipc/memory.rs` | 写入记忆 |
| 记忆 | `ic_memory_search` | `ipc/memory.rs` | 搜索记忆 |
| 技能 | `ic_list_skills` | `ipc/skills.rs` | 列出技能 |
| 技能 | `ic_install_skill` | `ipc/skills.rs` | 安装技能 |
| 扩展 | `ic_list_extensions` | `ipc/extensions.rs` | 列出扩展 |
| 扩展 | `ic_install_extension` | `ipc/extensions.rs` | 安装扩展 |
| 任务 | `ic_job_events` | `ipc/jobs.rs` | 获取任务事件 |
| 日程 | `ic_routine_runs` | `ipc/routines.rs` | 获取日程运行记录 |

#### 核心模块

| 模块 | 文件 | 职责 |
|------|------|------|
| Engine | `engine.rs` | 启动 IronClaw 引擎，初始化所有组件 |
| TauriChannel | `tauri_channel.rs` | 实现 `Channel` trait，桥接 Agent ↔ 前端 |
| AppState | `state.rs` | Tauri 全局状态（EngineState 状态机） |
| DLP Bridge | `dlp_integration.rs` | DLP 规则引擎集成 |
| Safety Bridge | `safety_bridge.rs` | 安全层桥接 |
| Admin Sync | `admin_sync.rs` | 定期从 Admin Backend 同步配置 |
| Embedded Server | `embedded_server.rs` | 内嵌 HTTP 服务器 (端口 38080) |

#### TauriChannel 工作原理

```rust
// 实现 ironclaw::channels::Channel trait
// 零开销进程内通信，无需 HTTP/SSE/WebSocket

// 消息入站：前端 invoke → mpsc::Sender → Agent
pub fn sender(&self) -> mpsc::Sender<IncomingMessage>

// 消息出站：Agent → app_handle.emit("chat-event") → 前端 listen
fn emit_event(&self, event: &ChatEvent) -> Result<(), ChannelError>
```

---

### 3.3 Admin Backend (Rust + React)

| 项目 | 说明 |
|------|------|
| 位置 | `admin-backend/` |
| 后端框架 | Axum + Tokio |
| 前端框架 | React 18 + TypeScript + Vite |
| 数据库 | PostgreSQL (通过 deadpool-postgres) |
| 认证 | JWT (通过 ironclaw_auth 共享 crate) |
| 端口 | 后端 3000, 前端 5174 |

#### API 路由清单

| 分类 | 方法 | 路径 | Handler | 说明 |
|------|------|------|---------|------|
| 健康检查 | GET | `/health` | `health_check` | 服务健康状态 |
| **对话代理** | **POST** | **`/api/chat/completions`** | **`chat_completions_handler`** | **核心：转发 LLM 请求** |
| 认证 | POST | `/api/auth/register` | `register` | 用户注册 |
| 认证 | POST | `/api/auth/login` | `login` | 用户登录，返回 JWT |
| 认证 | POST | `/api/auth/refresh` | `refresh_token` | 刷新 Token |
| 用户 | GET | `/api/users` | `get_users` | 用户列表 |
| 用户 | POST | `/api/users` | `create_user` | 创建用户 |
| 用户 | PUT | `/api/users/:id` | `update_user` | 更新用户 |
| 用户 | DELETE | `/api/users/:id` | `delete_user` | 删除用户 |
| DLP 规则 | GET | `/api/dlp-rules` | `get_dlp_rules` | 规则列表 |
| DLP 规则 | POST | `/api/dlp-rules` | `create_dlp_rule` | 创建规则 |
| DLP 规则 | PUT | `/api/dlp-rules/:id` | `update_dlp_rule` | 更新规则 |
| DLP 规则 | DELETE | `/api/dlp-rules/:id` | `delete_dlp_rule` | 删除规则 |
| DLP 批量 | POST | `/api/dlp-rules/batch-status` | `batch_update_dlp_status` | 批量启停 |
| DLP 批量 | POST | `/api/dlp-rules/batch-delete` | `batch_delete_dlp_rules` | 批量删除 |
| DLP 导入导出 | GET | `/api/dlp-rules/export` | `export_dlp_rules` | 导出规则 |
| DLP 导入导出 | POST | `/api/dlp-rules/import` | `import_dlp_rules` | 导入规则 |
| 敏感操作 | CRUD | `/api/sensitive-operations` | `get/create/update/delete_sensitive_operation` | 敏感操作管理 |
| 字典 | CRUD | `/api/dictionaries` | `get/create/update/delete_dictionary` | DLP 字典管理 |
| 角色 | CRUD | `/api/roles` | `get/create/update/delete_role` | RBAC 角色管理 |
| 权限 | GET/POST | `/api/permissions` | `get_permissions`, `assign_permissions` | 权限管理 |
| 策略变更 | GET | `/api/policy-changes` | `get_policy_changes` | 策略变更历史 |
| 策略变更 | GET | `/api/policy-changes/stats` | `get_policy_change_stats` | 变更统计 |
| 客户端 | GET | `/api/clients` | `get_clients` | 已连接客户端列表 |
| 客户端 | GET | `/api/clients/:id` | `get_client_detail` | 客户端详情 |
| 客户端 | DELETE | `/api/clients/:id` | `delete_client_record` | 删除客户端记录 |
| 客户端 | POST | `/api/clients/:id/disconnect` | `disconnect_client` | 断开客户端 |
| 客户端配置 | GET | `/api/client-config` | `get_client_config` | 客户端拉取配置 |
| 客户端配置 | PUT | `/api/client-config` | `update_client_config` | 更新客户端配置 |
| 策略推送 | POST | `/api/clients/:id/push-policy` | `push_policy_to_client` | 推送策略到单个客户端 |
| 策略推送 | POST | `/api/clients/push-policy-all` | `push_policy_all` | 推送策略到所有客户端 |
| 模型配置 | GET | `/api/model-configs` | `get_model_configs` | 模型配置列表 |
| 模型配置 | POST | `/api/model-configs` | `create_model_config` | 创建模型配置 |
| 模型配置 | PUT | `/api/model-configs/:id` | `update_model_config` | 更新模型配置 |
| 模型配置 | DELETE | `/api/model-configs/:id` | `delete_model_config` | 删除模型配置 |
| 客户端模型 | GET | `/api/client-models` | `get_client_models` | 客户端可用模型 |
| 技能 | GET | `/api/skills` | `get_skills` | 技能列表 |
| 技能 | POST | `/api/skills/:id/toggle` | `toggle_skill` | 启停技能 |
| 插件 | GET | `/api/plugins` | `get_plugins` | 插件列表 |
| 插件 | POST | `/api/plugins/:id/toggle` | `toggle_plugin` | 启停插件 |
| 部门 | CRUD | `/api/departments` | `get/create/update/delete_department` | 部门管理 |
| 审计日志 | GET | `/api/audit-logs` | `get_audit_logs` | 审计日志查询 |
| 审计日志 | GET | `/api/audit-logs/export` | `export_audit_logs` | 导出审计日志 |
| 审计上报 | POST | `/api/audit-events` | `report_audit_event` | 客户端上报审计事件 |
| 客户端上报 | POST | `/api/client-reports` | `post_client_reports` | 客户端上报数据 |
| 仪表板 | GET | `/api/dashboard/stats` | `get_dashboard_stats` | 仪表板统计 |
| 仪表板 | GET | `/api/dashboard/activity` | `get_dashboard_activity` | 活动数据 |
| 仪表板 | GET | `/api/dashboard/trends` | `get_dashboard_trends` | 趋势数据 |
| 设置 | GET/PUT | `/api/settings` | `get/update_settings` | 系统设置 |

#### Chat Proxy 核心逻辑

`chat_completions_handler` 是对话流的关键中转：

```
前端请求 (Vercel AI SDK 格式)
  │
  ▼
反序列化 ChatCompletionRequest
  │ 兼容两种 content 格式:
  │ - string: {"role":"user","content":"hello"}
  │ - parts:  {"role":"user","parts":[{"type":"text","text":"hello"}]}
  │
  ▼
查询 model_configs 表
  │ SELECT api_base_url, api_key, provider
  │ FROM model_configs WHERE model_id = $1 AND enabled = true
  │
  ▼
创建 LLM Provider (ironclaw_llm crate)
  │ provider → protocol 映射:
  │   "anthropic" → Anthropic API
  │   "ollama"    → Ollama 本地
  │   其他        → OpenAI 兼容 (DeepSeek, 通义千问等)
  │
  ▼
调用 llm.complete(request)
  │
  ▼
编码为 Vercel AI SDK Data Stream 返回
  {"type":"text-start","id":"msg-uuid"}
  {"type":"text-delta","id":"msg-uuid","delta":"回复内容"}
  {"type":"text-end","id":"msg-uuid"}
```

---

### 3.4 IronClaw 核心引擎

| 项目 | 说明 |
|------|------|
| 位置 | `ironclaw/src/` |
| 语言 | Rust |
| 数据库 | PostgreSQL 15+ (pgvector) |
| 异步运行时 | Tokio |

#### 核心模块

| 模块 | 目录 | 职责 | 关键工具/库 |
|------|------|------|------------|
| Agent | `src/agent/` | 主消息循环，意图路由，LLM 推理，工具编排 | tokio, ironclaw_llm |
| Channels | `src/channels/` | 多通道输入系统 | axum (Web), rustyline (REPL), wasmtime (WASM) |
| Worker | `src/worker/` | 后台任务执行引擎 | tokio 任务池 |
| Orchestrator | `src/orchestrator/` | Docker 容器生命周期管理 | bollard (Docker API) |
| Safety | `src/safety/` | 提示注入防御、内容清理、DLP | 正则引擎, 模式匹配 |
| Sandbox | `src/sandbox/` | WASM 沙箱，能力隔离 | wasmtime |
| Tools | `src/tools/` | 工具注册表 (MCP/WASM/内置) | serde_json |
| Workspace | `src/workspace/` | 持久化记忆，混合搜索 | PostgreSQL + pgvector |
| Extensions | `src/extensions/` | 动态扩展管理 | WASM 模块加载 |
| Skills | `src/skills/` | 技能库管理 | 文件系统 + 数据库 |
| Routines | `src/webhooks/` | 定时任务、事件触发、Webhook | cron 调度 |

#### 通道系统 (Channel Trait)

所有通道实现统一的 `Channel` trait：

```rust
#[async_trait]
pub trait Channel: Send + Sync {
    fn name(&self) -> &str;
    async fn start(&self) -> Result<MessageStream, ChannelError>;
    async fn respond(&self, response: OutgoingResponse) -> Result<(), ChannelError>;
    async fn send_status(&self, status: StatusUpdate) -> Result<(), ChannelError>;
    async fn broadcast(&self, message: &str) -> Result<(), ChannelError>;
    async fn health_check(&self) -> Result<(), ChannelError>;
}
```

| 通道 | 文件 | 传输协议 | 用途 |
|------|------|---------|------|
| Web Gateway | `channels/web/` | HTTP + SSE | 浏览器 Web UI |
| Tauri | `desktop-client/src/tauri_channel.rs` | Tauri IPC (进程内) | 桌面客户端 |
| REPL | `channels/repl.rs` | stdin/stdout | CLI 交互 |
| HTTP | `channels/http.rs` | HTTP POST | Webhook 接收 |
| Signal | `channels/signal.rs` | SSE (Signal CLI) | Signal 消息 |
| Relay | `channels/relay/` | WebSocket | Slack/Telegram 等 |
| WASM | `channels/wasm/` | WASM 沙箱 | 第三方集成 |

#### Web Gateway API

| 方法 | 路径 | Handler | 说明 |
|------|------|---------|------|
| POST | `/api/chat/send` | `chat_send_handler` | 发送消息 |
| POST | `/api/chat/approve` | `chat_approval_handler` | 工具审批 |
| POST | `/api/chat/auth-token` | `chat_auth_token_handler` | 获取认证 Token |
| GET | `/api/memory/tree` | `memory_tree_handler` | 记忆树 |
| GET | `/api/memory/list` | `memory_list_handler` | 记忆列表 |
| GET | `/api/memory/read` | `memory_read_handler` | 读取记忆 |
| POST | `/api/memory/write` | `memory_write_handler` | 写入记忆 |
| GET | `/api/memory/search` | `memory_search_handler` | 搜索记忆 |
| GET | `/api/jobs` | `jobs_list_handler` | 任务列表 |
| GET | `/api/jobs/:id` | `jobs_detail_handler` | 任务详情 |
| POST | `/api/jobs/:id/cancel` | `jobs_cancel_handler` | 取消任务 |
| POST | `/api/jobs/:id/restart` | `jobs_restart_handler` | 重启任务 |
| GET | `/api/extensions` | `extensions_list_handler` | 扩展列表 |
| POST | `/api/extensions/install` | `extensions_install_handler` | 安装扩展 |
| DELETE | `/api/extensions/:id` | `extensions_remove_handler` | 卸载扩展 |
| GET | `/api/skills` | `skills_list_handler` | 技能列表 |
| GET | `/api/skills/search` | `skills_search_handler` | 搜索技能 |
| POST | `/api/skills/install` | `skills_install_handler` | 安装技能 |
| GET | `/api/routines` | `routines_list_handler` | 日程列表 |
| POST | `/api/routines/:id/trigger` | `routines_trigger_handler` | 触发日程 |
| GET | `/api/settings` | `settings_list_handler` | 设置列表 |
| PUT | `/api/settings/:key` | `settings_set_handler` | 更新设置 |

---

### 3.5 共享 Crates

| Crate | 位置 | 职责 | 使用方 |
|-------|------|------|--------|
| `ironclaw_auth` | `crates/ironclaw_auth/` | JWT 生成/验证、密码哈希 (Argon2) | Admin Backend, Desktop Client |
| `ironclaw_llm` | `crates/ironclaw_llm/` | LLM 提供商抽象层 | Admin Backend (Chat Proxy), IronClaw Engine |

#### ironclaw_llm 支持的 Provider

| Provider | 协议 | 实现文件 | 说明 |
|----------|------|---------|------|
| OpenAI 兼容 | OpenAI Completions API | `openai_compat.rs` | DeepSeek, 通义千问, Moonshot 等 |
| Anthropic | Anthropic Messages API | `anthropic_oauth.rs` | Claude 系列 |
| Ollama | Ollama API | `ollama.rs` | 本地模型 |
| AWS Bedrock | AWS SDK | `bedrock.rs` | AWS 托管模型 |

关键能力：
- `create_provider_from_config()` — 根据配置创建 Provider
- `build_provider_chain()` — 构建 Provider 链（主 + 降级）
- `CircuitBreaker` — 熔断器，自动降级到备用 Provider

---

## 4. 协议汇总

| 通信路径 | 协议 | 格式 | 方向 |
|----------|------|------|------|
| 前端 → Tauri Rust | Tauri IPC | JSON (serde 序列化) | 双向同步 (invoke/return) |
| Tauri Rust → 前端 | Tauri Event | JSON (`ChatEvent` tagged enum) | 单向推送 (emit) |
| 前端 → Admin Backend | HTTP POST | Vercel AI SDK Data Stream | 请求-响应 |
| Admin Backend → LLM | HTTP POST | OpenAI/Anthropic/Ollama API | 请求-响应 |
| Desktop → Admin Backend | HTTP GET | JSON | 配置同步 (DLP 规则等) |
| 浏览器 → Web Gateway | HTTP + SSE | JSON | 双向 (POST 发送, SSE 接收) |
| Agent ↔ TauriChannel | mpsc channel | Rust struct (零拷贝) | 进程内双向 |
| Agent ↔ Web Gateway | mpsc channel | Rust struct | 进程内双向 |
| Signal CLI → IronClaw | SSE | JSON (Signal Envelope) | 单向接收 |
| WASM 通道 | WASM ABI | 序列化 bytes | 沙箱隔离 |

---

## 5. DLP 安全层数据流

```
用户输入 "我的身份证号是 310xxx"
  │
  ▼
前端 invoke('scan_user_input', { content })
  │
  ▼
ipc/dlp.rs → DLP 引擎扫描
  │
  ├─ 规则来源 1: 内置规则 (编译时)
  │   - PII 模式 (身份证、手机号、邮箱、银行卡)
  │   - 密钥模式 (API Key, AWS Secret)
  │
  ├─ 规则来源 2: Admin Backend 同步规则
  │   invoke('sync_dlp_rules_from_admin')
  │   → HTTP GET /api/dlp-rules
  │   → 合并到本地规则集
  │
  ▼
返回 DlpScanResponse:
  ├─ action: "clean"     → 放行
  ├─ action: "redacted"  → 返回脱敏内容 "我的身份证号是 [身份证号]"
  │   + stats: { pii_count: 1, types: ["id_card"] }
  └─ action: "blocked"   → 阻止发送
      + reason: "检测到密钥信息"
  │
  ▼
前端根据 action 决定:
  - clean:    正常发送到 LLM
  - redacted: 用脱敏内容替换原文，显示 DlpWarningBanner
  - blocked:  弹出 DlpBlockedDialog，不发送
```

---

## 6. 启动流程

### Desktop Client

```
main.rs
  │
  ├─ Tauri::Builder::default()
  │   .setup(|app| {
  │       // 1. 初始化 EngineState (NotStarted)
  │       // 2. 启动 admin_sync 定时任务
  │       // 3. 异步启动 start_ironclaw_engine()
  │   })
  │   .invoke_handler(all_tauri_commands!())
  │
  ▼
engine.rs::start_ironclaw_engine()
  │
  ├─ Config::from_env()           // 加载配置
  ├─ AppBuilder::build_all()      // 初始化所有组件
  ├─ TauriChannel::new()          // 创建 IPC 通道
  ├─ ChannelManager::new()        // 通道管理器
  ├─ Agent::new(deps)             // 创建 Agent
  ├─ sync_dlp_rules()             // 同步 DLP 规则
  ├─ EngineState → Ready          // 状态转换
  └─ agent.run()                  // 启动消息循环
```

### Admin Backend

```
main.rs
  │
  ├─ dotenv::dotenv()             // 加载 .env
  ├─ 连接 PostgreSQL              // deadpool-postgres
  ├─ 运行迁移                     // SQL 迁移文件
  ├─ create_router(AppState)      // 创建 Axum 路由
  └─ axum::serve(listener, app)   // 启动 HTTP 服务器 :3000
```

---

## 7. 关键设计决策

| 决策 | 原因 |
|------|------|
| Desktop Client 内嵌完整 IronClaw 引擎 | 离线可用，无需外部服务依赖 |
| 对话走 Admin Backend Chat Proxy 而非直连 LLM | 集中管理 API Key、模型配置、审计日志 |
| DLP 在前端拦截而非后端 | 敏感数据不出客户端，零信任架构 |
| 使用 @assistant-ui 而非自建对话 UI | 成熟的 AI 对话 UI 框架，支持流式渲染 |
| Vercel AI SDK Data Stream 协议 | @assistant-ui 原生支持，无需自定义解析 |
| 共享 Crate 而非代码复制 | 避免 Admin Backend 和 Desktop Client 重复实现 |
| Channel trait 抽象 | 统一多通道接入，新增通道只需实现 trait |
| TauriChannel 进程内通信 | 零网络开销，比 HTTP/WebSocket 更高效 |

---

## 8. 文件位置速查

| 你想找... | 去这里 |
|-----------|--------|
| 前端对话 UI 组件 | `desktop-client/ui/src/app/components/ai/` |
| 前端运行时配置 | `desktop-client/ui/src/app/runtime/ChatRuntimeProvider.tsx` |
| 前端 DLP hooks | `desktop-client/ui/src/app/hooks/useDlpScan.ts` |
| Tauri IPC 命令 | `desktop-client/src/ipc/*.rs` |
| Tauri 事件定义 | `desktop-client/src/tauri_channel.rs` (ChatEvent enum) |
| 引擎启动逻辑 | `desktop-client/src/engine.rs` |
| Admin Backend 路由 | `admin-backend/src/routes.rs` |
| Chat Proxy (LLM 转发) | `admin-backend/src/chat_proxy.rs` |
| LLM Provider 抽象 | `crates/ironclaw_llm/src/` |
| 认证共享库 | `crates/ironclaw_auth/src/` |
| Agent 消息循环 | `ironclaw/src/agent/` |
| 通道系统 | `ironclaw/src/channels/` |
| Web Gateway API | `ironclaw/src/channels/web/handlers/` |
| 安全/DLP 引擎 | `ironclaw/src/safety/` |
| 数据库迁移 | `ironclaw/migrations/`, `admin-backend/migrations/` |
