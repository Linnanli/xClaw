# IronClaw 库嵌入客户端改造计划

> 创建日期：2026-03-21
> 目标：将 IronClaw 作为库嵌入客户端，实现"双击即用"的桌面 AI 助手
> 设计原则：从零设计最合理的架构，不考虑历史包袱

---

## 一、目标架构

```
Desktop Client 进程
  │
  ├── Tauri Shell（窗口管理、系统托盘）
  │
  ├── IronClaw Core（库嵌入）
  │   ├── AppBuilder::build_all() → AppComponents
  │   ├── Agent（AI 对话引擎）
  │   ├── ToolRegistry（工具系统）
  │   ├── SafetyLayer（安全过滤）
  │   ├── Workspace（记忆/工作空间）
  │   ├── SkillRegistry（技能系统）
  │   ├── ExtensionManager（扩展管理）
  │   └── libSQL（本地数据库）
  │
  ├── TauriChannel（新增，实现 ironclaw::Channel trait）
  │   ├── 接收前端消息 → 转为 IncomingMessage → 送入 Agent
  │   └── 接收 Agent 事件 → 转为 Tauri Event → 推送到前端
  │
  ├── AdminSync（管理端同步）
  │   ├── 启动时拉取配置（LLM Key、安全策略等）
  │   └── 运行时上报数据（审计日志、DLP 事件等）
  │
  └── React 前端
      ├── Tauri invoke() → 发送消息、管理技能/扩展等
      └── Tauri listen() → 接收 AI 回复、状态更新等
```

### 核心设计：TauriChannel

不使用 Gateway（HTTP 服务），而是实现一个原生的 `TauriChannel`，
直接对接 IronClaw 的 `Channel` trait。这是最干净的集成方式：

```
前端 invoke("send_message")
  → TauriChannel.incoming_tx.send(IncomingMessage)
  → Agent 处理
  → TauriChannel.respond() / send_status()
  → app_handle.emit("chat-event", ...)
  → 前端 listen("chat-event")
```

**不需要 HTTP 服务、不需要 SSE、不需要 WebSocket。**
Tauri 的 IPC 就是最高效的进程内通信方式。


---

## 二、TauriChannel 实现设计

### 2.1 Channel Trait 接口

IronClaw 的 `Channel` trait（定义于 `ironclaw/src/channels/channel.rs`）要求实现：

| 方法 | 用途 | TauriChannel 实现策略 |
|------|------|----------------------|
| `name()` | 返回通道名 | 返回 `"tauri"` |
| `start()` | 返回消息流 | 返回 `incoming_rx` 的 Stream 包装 |
| `respond()` | 发送回复 | `app_handle.emit("chat-event", ChatEvent::Response{...})` |
| `send_status()` | 发送状态更新 | `app_handle.emit("chat-event", ChatEvent::Thinking/Status{...})` |
| `broadcast()` | 主动推送 | `app_handle.emit("chat-event", ...)` |
| `health_check()` | 健康检查 | 始终返回 `Ok(())` |
| `shutdown()` | 关闭通道 | 关闭 `incoming_tx` |

### 2.2 核心数据结构

```rust
// desktop-client/src/tauri_channel.rs

use std::sync::Arc;
use async_trait::async_trait;
use tokio::sync::mpsc;
use tauri::AppHandle;
use ironclaw::channels::{
    Channel, IncomingMessage, OutgoingResponse, StatusUpdate, MessageStream,
};
use ironclaw::error::ChannelError;

/// TauriChannel - 桥接 IronClaw Agent 和 Tauri 前端
///
/// 实现 IronClaw 的 Channel trait，将 Agent 的输出直接转为 Tauri IPC 事件。
/// 无需 HTTP/SSE/WebSocket，进程内零开销通信。
pub struct TauriChannel {
    app_handle: AppHandle,
    incoming_tx: mpsc::Sender<IncomingMessage>,
    incoming_rx: tokio::sync::Mutex<Option<mpsc::Receiver<IncomingMessage>>>,
    owner_id: String,
}

/// 前端接收的聊天事件（与现有前端 ChatEvent 类型完全兼容）
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type")]
pub enum ChatEvent {
    #[serde(rename = "response")]
    Response {
        message_id: String,
        content: String,
        thread_id: String,
    },
    #[serde(rename = "thinking")]
    Thinking { message: String },
    #[serde(rename = "status")]
    Status { message: String, level: String },
    #[serde(rename = "error")]
    Error {
        message: String,
        code: Option<String>,
    },
    #[serde(rename = "connection_status")]
    ConnectionStatus {
        connected: bool,
        message: String,
    },
    // === 扩展事件（前端可按需处理）===
    #[serde(rename = "tool_started")]
    ToolStarted { name: String },
    #[serde(rename = "tool_completed")]
    ToolCompleted {
        name: String,
        success: bool,
        error: Option<String>,
    },
    #[serde(rename = "stream_chunk")]
    StreamChunk { content: String },
    #[serde(rename = "approval_needed")]
    ApprovalNeeded {
        request_id: String,
        tool_name: String,
        description: String,
    },
    #[serde(rename = "image_generated")]
    ImageGenerated {
        data_url: String,
        path: Option<String>,
    },
    #[serde(rename = "suggestions")]
    Suggestions { suggestions: Vec<String> },
}
```


### 2.3 Channel Trait 实现

```rust
impl TauriChannel {
    pub fn new(app_handle: AppHandle, owner_id: String) -> Self {
        let (tx, rx) = mpsc::channel(64);
        Self {
            app_handle,
            incoming_tx: tx,
            incoming_rx: tokio::sync::Mutex::new(Some(rx)),
            owner_id,
        }
    }

    /// 获取消息发送端，供 Tauri Command 使用
    pub fn sender(&self) -> mpsc::Sender<IncomingMessage> {
        self.incoming_tx.clone()
    }

    /// 将 StatusUpdate 转为前端 ChatEvent
    fn status_to_event(status: &StatusUpdate) -> ChatEvent {
        match status {
            StatusUpdate::Thinking(msg) => ChatEvent::Thinking {
                message: msg.clone(),
            },
            StatusUpdate::ToolStarted { name } => ChatEvent::ToolStarted {
                name: name.clone(),
            },
            StatusUpdate::ToolCompleted {
                name,
                success,
                error,
                ..
            } => ChatEvent::ToolCompleted {
                name: name.clone(),
                success: *success,
                error: error.clone(),
            },
            StatusUpdate::StreamChunk(content) => ChatEvent::StreamChunk {
                content: content.clone(),
            },
            StatusUpdate::Status(msg) => ChatEvent::Status {
                message: msg.clone(),
                level: "info".to_string(),
            },
            StatusUpdate::ApprovalNeeded {
                request_id,
                tool_name,
                description,
                ..
            } => ChatEvent::ApprovalNeeded {
                request_id: request_id.clone(),
                tool_name: tool_name.clone(),
                description: description.clone(),
            },
            StatusUpdate::ImageGenerated { data_url, path } => ChatEvent::ImageGenerated {
                data_url: data_url.clone(),
                path: path.clone(),
            },
            StatusUpdate::Suggestions { suggestions } => ChatEvent::Suggestions {
                suggestions: suggestions.clone(),
            },
            // JobStarted, AuthRequired, AuthCompleted, ToolResult
            // → 统一映射为 Status 事件
            _ => ChatEvent::Status {
                message: format!("{:?}", status),
                level: "debug".to_string(),
            },
        }
    }
}

#[async_trait]
impl Channel for TauriChannel {
    fn name(&self) -> &str {
        "tauri"
    }

    async fn start(&self) -> Result<MessageStream, ChannelError> {
        let rx = self.incoming_rx.lock().await.take().ok_or_else(|| {
            ChannelError::StartupFailed {
                name: "tauri".to_string(),
                reason: "Channel already started".to_string(),
            }
        })?;

        // 发送连接成功事件
        let _ = self.app_handle.emit("chat-event", ChatEvent::ConnectionStatus {
            connected: true,
            message: "IronClaw engine ready".to_string(),
        });

        let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
        Ok(Box::pin(stream))
    }

    async fn respond(
        &self,
        msg: &IncomingMessage,
        response: OutgoingResponse,
    ) -> Result<(), ChannelError> {
        let event = ChatEvent::Response {
            message_id: msg.id.to_string(),
            content: response.content,
            thread_id: msg.thread_id.clone().unwrap_or_default(),
        };
        self.app_handle
            .emit("chat-event", event)
            .map_err(|e| ChannelError::SendFailed {
                name: "tauri".to_string(),
                reason: e.to_string(),
            })
    }

    async fn send_status(
        &self,
        status: StatusUpdate,
        _metadata: &serde_json::Value,
    ) -> Result<(), ChannelError> {
        let event = Self::status_to_event(&status);
        self.app_handle
            .emit("chat-event", event)
            .map_err(|e| ChannelError::SendFailed {
                name: "tauri".to_string(),
                reason: e.to_string(),
            })
    }

    async fn broadcast(
        &self,
        _user_id: &str,
        response: OutgoingResponse,
    ) -> Result<(), ChannelError> {
        let event = ChatEvent::Response {
            message_id: uuid::Uuid::new_v4().to_string(),
            content: response.content,
            thread_id: response.thread_id.unwrap_or_default(),
        };
        self.app_handle
            .emit("chat-event", event)
            .map_err(|e| ChannelError::SendFailed {
                name: "tauri".to_string(),
                reason: e.to_string(),
            })
    }

    async fn health_check(&self) -> Result<(), ChannelError> {
        Ok(()) // 进程内通信，始终健康
    }

    async fn shutdown(&self) -> Result<(), ChannelError> {
        let _ = self.app_handle.emit("chat-event", ChatEvent::ConnectionStatus {
            connected: false,
            message: "IronClaw engine shutting down".to_string(),
        });
        Ok(())
    }
}
```

### 2.4 前端兼容性

现有前端 `useAiChatTauri.ts` 已经使用 `listen<ChatEvent>('chat-event', ...)` 接收事件，
`ChatEvent` 类型定义完全匹配。**前端代码零修改。**

唯一变化：不再需要 `subscribe_chat_events` / `unsubscribe_chat_events` 命令，
因为 TauriChannel 在 Agent 启动时自动开始推送事件。前端的 `listen()` 天然就是订阅。


---

## 三、客户端启动流程

### 3.1 启动时序

```
Tauri main()
  │
  ├── 1. 加载本地缓存配置（上次管理端下发的配置）
  │
  ├── 2. AdminSync: 尝试拉取最新配置（LLM Key、安全策略等）
  │   ├── 成功 → 更新本地缓存，使用新配置
  │   └── 失败 → 使用本地缓存（离线可用）
  │
  ├── 3. 注入环境变量（LLM_API_KEY、LLM_BACKEND 等）
  │   └── std::env::set_var() — 在 Tokio runtime 启动前执行，安全
  │
  ├── 4. Config::from_env() → 加载 IronClaw 配置
  │
  ├── 5. AppBuilder::build_all() → 初始化所有组件
  │   ├── Phase 1: init_database() — libSQL 本地数据库
  │   ├── Phase 2: init_secrets() — 密钥存储
  │   ├── Phase 3: init_llm() — LLM 提供者链
  │   ├── Phase 4: init_tools() — 工具注册
  │   └── Phase 5: init_extensions() — 扩展/MCP/WASM
  │
  ├── 6. 创建 TauriChannel + ChannelManager
  │
  ├── 7. 构建 AgentDeps + Agent::new()
  │
  ├── 8. tokio::spawn(agent.run()) — 后台运行 Agent
  │
  ├── 9. tokio::spawn(admin_sync_loop()) — 后台数据上报
  │
  └── 10. Tauri 窗口就绪，前端加载
```

### 3.2 启动代码骨架

```rust
// desktop-client/src/main.rs

fn main() {
    // ⚠️ 在 Tokio runtime 启动前注入环境变量（安全）
    let _ = dotenvy::dotenv();
    inject_admin_config_to_env();

    tauri::Builder::default()
        .setup(|app| {
            let app_handle = app.handle().clone();
            
            // 在 Tauri 的异步上下文中启动 IronClaw
            tauri::async_runtime::spawn(async move {
                if let Err(e) = start_ironclaw_engine(app_handle).await {
                    tracing::error!("IronClaw engine failed: {}", e);
                }
            });
            
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // 聊天
            send_chat_message,
            // 线程管理
            list_threads,
            create_thread,
            get_thread_history,
            // 工具审批
            approve_tool,
            deny_tool,
            // 记忆/工作空间
            memory_list,
            memory_read,
            memory_write,
            memory_search,
            memory_delete,
            // 技能
            list_skills,
            install_skill,
            uninstall_skill,
            // 扩展
            list_extensions,
            install_extension,
            uninstall_extension,
            // 设置
            get_config,
            update_config,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// 注入管理端配置到环境变量
fn inject_admin_config_to_env() {
    if let Ok(config) = AdminConfigCache::load() {
        if let Some(key) = &config.llm_api_key {
            std::env::set_var("LLM_API_KEY", key);
        }
        if let Some(backend) = &config.llm_backend {
            std::env::set_var("LLM_BACKEND", backend);
        }
        if let Some(model) = &config.llm_model {
            std::env::set_var("LLM_MODEL", model);
        }
        // ... 其他配置项
    }
}

/// 启动 IronClaw 引擎
async fn start_ironclaw_engine(app_handle: AppHandle) -> anyhow::Result<()> {
    use ironclaw::app::{AppBuilder, AppBuilderFlags};
    use ironclaw::channels::ChannelManager;
    use ironclaw::config::Config;
    use ironclaw::llm::create_session_manager;
    use ironclaw::channels::web::log_layer::LogBroadcaster;

    // 1. 加载配置
    let config = Config::from_env().await?;
    let session = create_session_manager(config.llm.session.clone()).await;
    let log_broadcaster = Arc::new(LogBroadcaster::new());

    // 2. 构建所有组件
    let flags = AppBuilderFlags { no_db: false };
    let components = AppBuilder::new(config, flags, None, session, log_broadcaster)
        .build_all()
        .await?;

    let config = components.config.clone();

    // 3. 创建 TauriChannel
    let tauri_channel = TauriChannel::new(
        app_handle.clone(),
        config.owner_id.clone(),
    );
    let msg_sender = tauri_channel.sender();

    // 4. 注册到 ChannelManager
    let channels = ChannelManager::new();
    channels.add(Box::new(tauri_channel)).await;
    let channels = Arc::new(channels);

    // 5. 将 msg_sender 存入 Tauri State，供 Tauri Command 使用
    app_handle.manage(AppState {
        msg_sender,
        components_ref: /* ... */,
    });

    // 6. 构建 Agent
    let deps = AgentDeps { /* 从 components 构建 */ };
    let agent = Agent::new(
        config.agent.clone(),
        deps,
        channels,
        Some(config.heartbeat.clone()),
        Some(config.hygiene.clone()),
        Some(config.routines.clone()),
        Some(components.context_manager),
        Some(components.agent_session_manager),
    );

    // 7. 运行 Agent（阻塞直到关闭）
    agent.run().await?;

    Ok(())
}
```


---

## 四、Tauri Commands 设计

### 4.1 设计原则

旧架构：`Tauri Command → ApiClient (HTTP) → Gateway → Agent`
新架构：`Tauri Command → AppComponents 直接调用`

所有 Tauri Command 直接操作 `AppComponents` 中的组件，零网络开销。

### 4.2 AppState 定义

```rust
// desktop-client/src/state.rs

use std::sync::Arc;
use tokio::sync::mpsc;
use ironclaw::channels::IncomingMessage;
use ironclaw::workspace::Workspace;
use ironclaw::tools::ToolRegistry;
use ironclaw::extensions::ExtensionManager;
use ironclaw::skills::SkillRegistry;
use ironclaw::skills::catalog::SkillCatalog;
use ironclaw::db::Database;
use ironclaw::safety::SafetyLayer;

/// Tauri 全局状态 — 持有 IronClaw 组件的引用
pub struct AppState {
    /// 消息发送端 → TauriChannel → Agent
    pub msg_sender: mpsc::Sender<IncomingMessage>,
    /// 数据库（线程管理、记忆存储等）
    pub db: Option<Arc<dyn Database>>,
    /// 工作空间（记忆系统）
    pub workspace: Option<Arc<Workspace>>,
    /// 工具注册表
    pub tools: Arc<ToolRegistry>,
    /// 扩展管理器
    pub extension_manager: Option<Arc<ExtensionManager>>,
    /// 技能注册表
    pub skill_registry: Option<Arc<std::sync::RwLock<SkillRegistry>>>,
    /// 技能目录
    pub skill_catalog: Option<Arc<SkillCatalog>>,
    /// 安全层（DLP）
    pub safety: Arc<SafetyLayer>,
    /// 配置
    pub owner_id: String,
}
```

### 4.3 核心 Tauri Commands

#### 聊天命令

```rust
/// 发送聊天消息
#[tauri::command]
async fn send_chat_message(
    state: tauri::State<'_, AppState>,
    thread_id: String,
    content: String,
) -> Result<SendMessageResponse, String> {
    let msg = IncomingMessage::new("tauri", &state.owner_id, &content)
        .with_thread(&thread_id)
        .with_owner_id(&state.owner_id);

    state.msg_sender.send(msg).await
        .map_err(|e| format!("Failed to send message: {}", e))?;

    Ok(SendMessageResponse {
        message_id: uuid::Uuid::new_v4().to_string(),
        success: true,
    })
}
```

#### 记忆/工作空间命令

```rust
/// 列出记忆文件
#[tauri::command]
async fn memory_list(
    state: tauri::State<'_, AppState>,
    path: Option<String>,
) -> Result<Vec<MemoryEntry>, String> {
    let ws = state.workspace.as_ref()
        .ok_or("Workspace not available")?;
    
    ws.list_tree(path.as_deref())
        .await
        .map_err(|e| e.to_string())
}

/// 搜索记忆
#[tauri::command]
async fn memory_search(
    state: tauri::State<'_, AppState>,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<SearchResult>, String> {
    let ws = state.workspace.as_ref()
        .ok_or("Workspace not available")?;
    
    ws.search(&query, limit.unwrap_or(10))
        .await
        .map_err(|e| e.to_string())
}
```

#### 技能命令

```rust
/// 列出已安装技能
#[tauri::command]
async fn list_skills(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<SkillInfo>, String> {
    let registry = state.skill_registry.as_ref()
        .ok_or("Skills not enabled")?;
    let guard = registry.read().map_err(|e| e.to_string())?;
    
    Ok(guard.skills().iter().map(|s| SkillInfo {
        name: s.name().to_string(),
        description: s.description().to_string(),
        // ...
    }).collect())
}
```

#### 扩展命令

```rust
/// 列出扩展
#[tauri::command]
async fn list_extensions(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ExtensionInfo>, String> {
    let ext_mgr = state.extension_manager.as_ref()
        .ok_or("Extension manager not available")?;
    
    ext_mgr.list_all()
        .await
        .map_err(|e| e.to_string())
}

/// 安装扩展
#[tauri::command]
async fn install_extension(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<InstallResult, String> {
    let ext_mgr = state.extension_manager.as_ref()
        .ok_or("Extension manager not available")?;
    
    ext_mgr.install(&name)
        .await
        .map_err(|e| e.to_string())
}
```

### 4.4 命令对照表

| 功能 | 旧架构（HTTP 转发） | 新架构（直接调用） |
|------|---------------------|-------------------|
| 发送消息 | `POST /api/chat` → Gateway | `msg_sender.send(IncomingMessage)` |
| 获取线程 | `GET /api/threads` → Gateway | `db.list_threads()` |
| 记忆列表 | `GET /api/memory/tree` → Gateway | `workspace.list_tree()` |
| 记忆搜索 | `GET /api/memory/search` → Gateway | `workspace.search()` |
| 技能列表 | `GET /api/skills` → Gateway | `skill_registry.read().skills()` |
| 扩展安装 | `POST /api/extensions/install` → Gateway | `extension_manager.install()` |
| 工具审批 | `POST /api/approve` → Gateway | `context_manager.approve()` |
| DLP 扫描 | 自实现 DLP 模块 | `safety.scan_input()` |


---

## 五、管理端配置同步模块

### 5.1 架构

```
Admin Backend                    Desktop Client
┌──────────────┐                ┌──────────────────────┐
│ GET /api/    │  ◄── HTTPS ──  │ AdminConfigSync       │
│ client-config│                │  ├── 启动时拉取       │
│              │                │  ├── 定时刷新(5min)   │
│              │                │  ├── 本地缓存         │
│              │                │  └── 注入 env vars    │
└──────────────┘                └──────────────────────┘
```

### 5.2 配置项

管理端可下发的配置：

```rust
/// 管理端下发的客户端配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminClientConfig {
    // === LLM 配置 ===
    pub llm_backend: Option<String>,       // "openai" | "anthropic" | "nearai" | ...
    pub llm_api_key: Option<String>,       // API Key（加密传输）
    pub llm_model: Option<String>,         // 模型名称
    pub llm_base_url: Option<String>,      // 自定义 API 端点

    // === 安全策略 ===
    pub safety_enabled: Option<bool>,
    pub dlp_rules: Option<Vec<DlpRule>>,   // DLP 规则（复用现有 DLP 策略同步）

    // === 功能开关 ===
    pub skills_enabled: Option<bool>,
    pub extensions_enabled: Option<bool>,
    pub sandbox_enabled: Option<bool>,
    pub builder_mode_enabled: Option<bool>,

    // === 限制 ===
    pub max_cost_per_day_cents: Option<u64>,
    pub max_actions_per_hour: Option<u64>,

    // === 版本 ===
    pub config_version: u64,               // 配置版本号，用于增量更新
    pub updated_at: String,
}
```

### 5.3 环境变量映射

```rust
impl AdminClientConfig {
    /// 将管理端配置注入为环境变量
    /// ⚠️ 必须在 Tokio runtime 启动前调用
    pub fn inject_to_env(&self) {
        if let Some(ref backend) = self.llm_backend {
            std::env::set_var("LLM_BACKEND", backend);
        }
        if let Some(ref key) = self.llm_api_key {
            std::env::set_var("LLM_API_KEY", key);
        }
        if let Some(ref model) = self.llm_model {
            std::env::set_var("LLM_MODEL", model);
        }
        if let Some(ref url) = self.llm_base_url {
            std::env::set_var("LLM_BASE_URL", url);
        }
        if let Some(enabled) = self.safety_enabled {
            std::env::set_var("SAFETY_ENABLED", enabled.to_string());
        }
        if let Some(enabled) = self.skills_enabled {
            std::env::set_var("SKILLS_ENABLED", enabled.to_string());
        }
        if let Some(cost) = self.max_cost_per_day_cents {
            std::env::set_var("MAX_COST_PER_DAY_CENTS", cost.to_string());
        }
        // ... 其他映射
    }
}
```

### 5.4 本地缓存

```rust
/// 配置缓存路径：~/.ironclaw-desktop/admin_config.json
impl AdminConfigCache {
    pub fn cache_path() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("ironclaw-desktop")
            .join("admin_config.json")
    }

    pub fn save(&self) -> Result<()> { /* 加密写入本地文件 */ }
    pub fn load() -> Result<Self> { /* 从本地文件加载 */ }
}
```

### 5.5 Admin Backend 新增 API

```rust
// admin-backend/src/routes.rs

/// GET /api/client-config
/// 返回客户端应使用的配置
async fn get_client_config(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AdminClientConfig>, ApiError> {
    // 1. 验证客户端身份（设备 token 或用户 token）
    let client_id = authenticate_client(&headers, &state)?;
    
    // 2. 从数据库加载该客户端/用户组的配置
    let config = state.db.get_client_config(&client_id).await?;
    
    // 3. 返回配置（API Key 加密传输）
    Ok(Json(config))
}
```


---

## 六、数据上报模块

### 6.1 架构

```
Desktop Client                   Admin Backend
┌──────────────────────┐        ┌──────────────────┐
│ DataReporter          │        │                  │
│  ├── 审计日志收集     │──POST──►│ POST /api/       │
│  ├── DLP 事件收集     │        │ client-reports   │
│  ├── 使用统计收集     │        │                  │
│  ├── 本地队列缓冲     │        │ 存入数据库       │
│  └── 批量上报(30s)   │        │ 管理端可查看     │
└──────────────────────┘        └──────────────────┘
```

### 6.2 上报数据类型

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClientReport {
    /// 审计日志：用户操作记录
    AuditLog {
        timestamp: String,
        user_id: String,
        action: String,        // "chat", "tool_use", "extension_install", ...
        details: serde_json::Value,
    },
    /// DLP 事件：敏感数据检测
    DlpEvent {
        timestamp: String,
        user_id: String,
        had_sensitive_data: bool,
        was_blocked: bool,
        rule_matches: Vec<String>,
        // ⚠️ 不上报原始内容，只上报统计信息
    },
    /// 使用统计：LLM 调用量、token 消耗等
    UsageStats {
        timestamp: String,
        user_id: String,
        period_start: String,
        period_end: String,
        total_messages: u64,
        total_tokens: u64,
        total_cost_cents: u64,
        model_usage: HashMap<String, u64>,  // model_name → token_count
    },
    /// 健康状态：客户端运行状态
    HealthStatus {
        timestamp: String,
        client_version: String,
        uptime_secs: u64,
        db_size_bytes: u64,
        active_extensions: Vec<String>,
    },
}
```

### 6.3 数据收集方式

数据来源于 IronClaw 的 Hook 系统，零侵入：

```rust
/// 注册数据收集 Hook
pub fn register_reporting_hooks(
    hooks: &Arc<HookRegistry>,
    reporter: Arc<DataReporter>,
) {
    // 监听所有出站消息 → 审计日志
    hooks.register(Arc::new(AuditLogHook {
        reporter: reporter.clone(),
    }));

    // 监听安全事件 → DLP 事件
    hooks.register(Arc::new(DlpEventHook {
        reporter: reporter.clone(),
    }));
}
```

### 6.4 本地队列 + 批量上报

```rust
pub struct DataReporter {
    queue: Arc<Mutex<Vec<ClientReport>>>,
    admin_url: String,
    client_token: String,
}

impl DataReporter {
    /// 后台上报循环
    pub async fn run_loop(&self) {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            self.flush().await;
        }
    }

    /// 批量上报
    async fn flush(&self) {
        let reports: Vec<ClientReport> = {
            let mut queue = self.queue.lock().unwrap();
            std::mem::take(&mut *queue)
        };

        if reports.is_empty() { return; }

        match reqwest::Client::new()
            .post(&format!("{}/api/client-reports", self.admin_url))
            .bearer_auth(&self.client_token)
            .json(&reports)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                tracing::debug!("Reported {} events to admin", reports.len());
            }
            _ => {
                // 上报失败，放回队列（下次重试）
                let mut queue = self.queue.lock().unwrap();
                queue.extend(reports);
                tracing::warn!("Failed to report to admin, will retry");
            }
        }
    }
}
```


---

## 七、文件变更清单

### 7.1 新增文件

| 文件 | 用途 |
|------|------|
| `desktop-client/src/tauri_channel.rs` | TauriChannel 实现（Channel trait） |
| `desktop-client/src/state.rs` | AppState 定义（持有 IronClaw 组件引用） |
| `desktop-client/src/engine.rs` | IronClaw 引擎启动逻辑 |
| `desktop-client/src/admin_sync.rs` | 管理端配置同步 |
| `desktop-client/src/data_reporter.rs` | 数据上报模块 |
| `desktop-client/src/commands/chat.rs` | 聊天相关 Tauri Commands |
| `desktop-client/src/commands/memory.rs` | 记忆/工作空间 Tauri Commands |
| `desktop-client/src/commands/skills.rs` | 技能管理 Tauri Commands |
| `desktop-client/src/commands/extensions.rs` | 扩展管理 Tauri Commands |
| `desktop-client/src/commands/settings.rs` | 设置管理 Tauri Commands |
| `desktop-client/src/commands/mod.rs` | Commands 模块入口 |
| `admin-backend/src/client_config.rs` | 客户端配置下发 API |
| `admin-backend/src/client_reports.rs` | 客户端数据上报 API |
| `admin-backend/migrations/0XX_client_config.sql` | 客户端配置表 |

### 7.2 删除文件（旧架构废弃）

| 文件 | 原因 |
|------|------|
| `desktop-client/src/api_client.rs` | 不再需要 HTTP 转发 |
| `desktop-client/src/embedded_server.rs` | 不再需要外部 IronClaw 进程 |
| `desktop-client/src/commands.rs` | 重构为 `commands/` 模块 |
| `desktop-client/src/sse_client.rs` | 不再需要 SSE 客户端 |
| `desktop-client/src/extension_manager.rs` | 直接使用 IronClaw ExtensionManager |
| `desktop-client/src/routine_manager.rs` | 直接使用 IronClaw RoutineEngine |
| `desktop-client/src/skill_manager.rs` | 直接使用 IronClaw SkillRegistry |
| `desktop-client/src/memory_manager.rs` | 直接使用 IronClaw Workspace |

### 7.3 修改文件

| 文件 | 变更内容 |
|------|---------|
| `desktop-client/src/main.rs` | 重写启动流程，嵌入 IronClaw |
| `desktop-client/Cargo.toml` | 确认 `ironclaw` 依赖，移除 `reqwest`（不再需要 HTTP 客户端） |
| `admin-backend/src/routes.rs` | 新增 `/api/client-config` 和 `/api/client-reports` 路由 |
| `admin-backend/src/models.rs` | 新增 `ClientConfig` 和 `ClientReport` 模型 |

### 7.4 不变文件

| 文件 | 原因 |
|------|------|
| `ironclaw/` 整个目录 | **IronClaw 零修改** |
| `desktop-client/src-ui/` 前端代码 | 前端已使用 `listen('chat-event')`，兼容 |
| `desktop-client/src/dlp/` | DLP 模块保留，可选择使用 IronClaw SafetyLayer 替代 |
| `desktop-client/src/auth.rs` | 本地认证（主密码）保留 |
| `desktop-client/src/storage.rs` | 本地加密存储保留 |


---

## 八、实施里程碑

### Phase 1：核心引擎嵌入（预计 3-5 天）

**目标**：IronClaw 作为库在客户端进程内运行，能发送消息并收到回复。

- [ ] 实现 `TauriChannel`（Channel trait）
- [ ] 实现 `engine.rs`（启动 AppBuilder + Agent）
- [ ] 实现 `state.rs`（AppState 定义）
- [ ] 实现 `commands/chat.rs`（send_chat_message）
- [ ] 重写 `main.rs`（新启动流程）
- [ ] 验证：前端发送消息 → Agent 处理 → 前端收到回复

### Phase 2：功能命令迁移（预计 3-5 天）

**目标**：所有 Tauri Commands 从 HTTP 转发改为直接调用。

- [ ] 实现 `commands/memory.rs`（记忆 CRUD + 搜索）
- [ ] 实现 `commands/skills.rs`（技能管理）
- [ ] 实现 `commands/extensions.rs`（扩展管理）
- [ ] 实现 `commands/settings.rs`（配置管理）
- [ ] 删除 `api_client.rs`、`embedded_server.rs` 等旧文件
- [ ] 验证：所有前端功能正常工作

### Phase 3：管理端集成（预计 2-3 天）

**目标**：客户端能从管理端拉取配置、上报数据。

- [ ] 实现 `admin_sync.rs`（配置拉取 + 缓存）
- [ ] 实现 `data_reporter.rs`（数据上报）
- [ ] Admin Backend 新增 `GET /api/client-config`
- [ ] Admin Backend 新增 `POST /api/client-reports`
- [ ] 验证：管理端修改 LLM Key → 客户端自动生效

### Phase 4：DLP 统一（预计 1-2 天）

**目标**：使用 IronClaw SafetyLayer 替代客户端自实现的 DLP。

- [ ] 评估 SafetyLayer 是否覆盖现有 DLP 功能
- [ ] 如果覆盖 → 删除 `desktop-client/src/dlp/`，使用 SafetyLayer
- [ ] 如果不覆盖 → 保留 DLP 模块，与 SafetyLayer 并行运行
- [ ] 验证：敏感数据检测和脱敏正常工作

### Phase 5：测试与优化（预计 2-3 天）

**目标**：全面测试，确保稳定性和性能。

- [ ] 单元测试：TauriChannel、AdminSync、DataReporter
- [ ] 集成测试：完整聊天流程
- [ ] 性能测试：启动时间、内存占用
- [ ] 离线测试：无网络时使用缓存配置启动
- [ ] 安全测试：API Key 不泄露、DLP 正常工作


---

## 九、验证清单

### 功能验证

- [ ] 双击启动客户端，无需额外安装/启动 IronClaw 服务
- [ ] 发送消息，收到 AI 回复（完整对话流程）
- [ ] 流式输出正常（StreamChunk 事件）
- [ ] 工具调用正常（ToolStarted → ToolCompleted 事件）
- [ ] 工具审批流程正常（ApprovalNeeded → approve/deny）
- [ ] 记忆系统正常（CRUD + 搜索）
- [ ] 技能系统正常（列表、安装、卸载）
- [ ] 扩展系统正常（列表、安装、卸载、MCP）
- [ ] 多线程对话正常

### 管理端验证

- [ ] 管理端下发 LLM API Key → 客户端使用新 Key
- [ ] 管理端下发安全策略 → 客户端 DLP 规则更新
- [ ] 客户端审计日志 → 管理端可查看
- [ ] 客户端 DLP 事件 → 管理端可查看
- [ ] 客户端使用统计 → 管理端可查看

### 离线/异常验证

- [ ] 无网络时使用缓存配置正常启动
- [ ] 管理端不可达时客户端正常运行
- [ ] 数据上报失败时本地队列缓冲，恢复后重试
- [ ] IronClaw 引擎崩溃时前端显示错误提示

### 性能验证

- [ ] 冷启动时间 < 5 秒
- [ ] 内存占用 < 500MB（空闲状态）
- [ ] 消息响应延迟 < 100ms（不含 LLM 推理时间）
- [ ] 二进制体积增量合理

### 安全验证

- [ ] LLM API Key 不出现在日志中
- [ ] LLM API Key 不出现在前端可访问的位置
- [ ] 管理端配置传输使用 HTTPS
- [ ] 本地缓存的配置加密存储
- [ ] DLP 扫描在所有路径上生效（包括失败路径）

---

## 十、关键设计决策总结

| 决策 | 选择 | 理由 |
|------|------|------|
| IronClaw 集成方式 | 库嵌入（非外部进程） | 单进程、零网络开销、部署简单 |
| 通信方式 | TauriChannel（Channel trait） | 原生 IPC、类型安全、无序列化开销 |
| 是否保留 Gateway | 不保留 | 桌面客户端不需要 HTTP 服务 |
| IronClaw 是否需要修改 | 不需要 | AppBuilder 已支持库模式 |
| 前端是否需要修改 | 不需要 | 已使用 `listen('chat-event')` |
| 配置管理 | env var 注入 | Config::from_env() 天然支持 |
| 数据上报 | Hook 系统 + 批量上报 | 零侵入、可靠、高效 |
| 离线支持 | 本地配置缓存 | 无网络时仍可使用 |
