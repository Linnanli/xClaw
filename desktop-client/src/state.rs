//! Tauri 全局状态 — 持有 IronClaw 组件的引用。
//!
//! ## 架构
//!
//! `EngineState` 在 Tauri `setup()` 中**同步**注入（`manage()`），
//! 内部使用 `OnceLock<AppState>` 延迟填充。引擎异步启动完成后，
//! 通过 `EngineState::initialize()` 设置真正的 `AppState`。
//!
//! 所有 Tauri Command 通过 `State<'_, EngineState>` 访问，
//! 调用 `engine_state.get()` 获取 `&AppState`。如果引擎尚未就绪，
//! 返回友好的错误信息而非 panic。
//!
//! ## 三态模型
//!
//! ```text
//! Starting ──initialize()──→ Ready
//!    │
//!    └──set_failed()──→ Failed(reason)
//! ```
//!
//! ## 启动时序
//!
//! ```text
//! main.rs setup()
//!   → app_handle.manage(EngineState::new())   // 同步注入（空壳）
//!   → spawn(start_ironclaw_engine)            // 异步启动
//!     → 成功: engine_state.initialize(app_state)
//!     → 失败: engine_state.set_failed(error_msg)
//!     → 前端收到 "engine ready" 或 "engine error" 事件
//! ```

use std::sync::{Arc, OnceLock, RwLock};

use ironclaw::agent::routine_engine::RoutineEngine;
use ironclaw::channels::web::log_layer::LogBroadcaster;
use ironclaw::channels::IncomingMessage;
use ironclaw::config::SkillsConfig;
use ironclaw::context::ContextManager;
use ironclaw::db::Database;
use ironclaw::extensions::ExtensionManager;
use ironclaw::safety::SafetyLayer;
use ironclaw::skills::catalog::SkillCatalog;
use ironclaw::skills::SkillRegistry;
use ironclaw::tools::ToolRegistry;
use ironclaw::workspace::Workspace;
use tokio::sync::mpsc;

use crate::safety_bridge::SafetyBridge;

/// IronClaw 引擎内部状态。
///
/// 持有 IronClaw `AppComponents` 中各组件的 `Arc` 引用，
/// 供 Tauri Command 直接调用，无需 HTTP 转发。
pub struct AppState {
    /// 消息发送端 → TauriChannel → Agent 消息循环。
    pub msg_sender: mpsc::Sender<IncomingMessage>,
    /// 数据库（线程管理、设置存储等）。
    pub db: Option<Arc<dyn Database>>,
    /// 工作空间（记忆系统）。
    pub workspace: Option<Arc<Workspace>>,
    /// 工具注册表。
    pub tools: Arc<ToolRegistry>,
    /// 扩展管理器。
    pub extension_manager: Option<Arc<ExtensionManager>>,
    /// 技能注册表。
    pub skill_registry: Option<Arc<std::sync::RwLock<SkillRegistry>>>,
    /// 技能目录。
    pub skill_catalog: Option<Arc<SkillCatalog>>,
    /// 技能系统配置（用于 skill 匹配参数）。
    pub skills_config: SkillsConfig,
    /// 安全层（DLP / 内容过滤）。
    pub safety: Arc<SafetyLayer>,
    /// 安全桥接器（统一 SafetyLayer + DLP 格式保留脱敏）。
    pub safety_bridge: Arc<SafetyBridge>,
    /// 上下文管理器（任务审批等）。
    pub context_manager: Arc<ContextManager>,
    /// 实例 owner ID。
    pub owner_id: String,
    /// LLM provider 引用（用于模型切换和查询可用模型列表）。
    ///
    /// 模型切换策略：
    /// 1. 优先 `set_model()` — 对支持的 provider（NearAI、Anthropic）直接切换
    /// 2. 失败时回退到 `model_override` — 通过 per-request `request.model` 注入
    ///
    /// 两种机制统一在 `send_chat_message` 中处理。
    pub llm: Arc<dyn ironclaw::llm::LlmProvider>,
    /// Per-request 模型覆盖（`set_model()` 不支持时的回退机制）。
    ///
    /// `send_chat_message` 写入，`ModelSwitchProvider` 读取。
    /// 使用 `Arc` 共享，`engine.rs` 中 `ModelSwitchProvider` 持有同一个引用。
    pub model_override: Arc<std::sync::RwLock<Option<String>>>,
    /// ModelSwitchProvider 引用（用于跨 provider 切换时替换底层 provider）。
    pub model_switch: Arc<crate::model_switch::ModelSwitchProvider>,
    /// 当前 provider 的 base URL（用于检测跨 provider 切换）。
    pub provider_base_url: std::sync::RwLock<String>,
    /// 初始 provider 引用（跨 provider 切换后恢复用）。
    pub initial_provider: Arc<dyn ironclaw::llm::LlmProvider>,
    /// 初始 provider 的 base URL（用于检测"切回初始 provider"）。
    pub initial_base_url: String,
    /// 日志广播器（用于 LogsTab 读取运行时日志）。
    pub log_broadcaster: Arc<LogBroadcaster>,
    /// 日志清空偏移量：`ic_clear_logs` 时记录当前日志数，后续查询跳过此前的条目。
    pub log_clear_offset: std::sync::atomic::AtomicUsize,
    /// Routine engine slot — 引擎就绪后填充，供 ic_fire_routine 使用。
    pub routine_engine_slot: Arc<tokio::sync::RwLock<Option<Arc<RoutineEngine>>>>,
}

/// `main.rs` 中创建的 `LogBroadcaster` 的 Tauri managed state 包装。
///
/// 在 `main()` 最开始创建，通过 `init_tracing` 注册 `WebLogLayer`，
/// 确保引擎启动前的日志也能被捕获。引擎启动时从此处取出，存入 `AppState`。
pub struct SharedLogBroadcaster(pub Arc<LogBroadcaster>);

/// Tauri managed state — 引擎就绪前安全的包装器。
///
/// 在 `setup()` 中同步注入 Tauri，解决引擎异步启动导致的
/// "state not managed" 竞态条件。
///
/// ## 三态模型
///
/// - `Starting` — 引擎正在启动（`inner` 为空，`failure` 为空）
/// - `Ready` — 引擎就绪（`inner` 已填充）
/// - `Failed` — 引擎启动失败（`failure` 已填充）
pub struct EngineState {
    inner: OnceLock<AppState>,
    /// 引擎启动失败的原因。
    failure: RwLock<Option<String>>,
}

impl EngineState {
    /// 创建空的引擎状态（引擎尚未就绪）。
    pub fn new() -> Self {
        Self {
            inner: OnceLock::new(),
            failure: RwLock::new(None),
        }
    }

    /// 引擎启动完成后填充真正的状态。
    ///
    /// 只能调用一次，重复调用返回 `Err`。
    pub fn initialize(&self, state: AppState) -> Result<(), String> {
        self.inner
            .set(state)
            .map_err(|_| "EngineState already initialized".to_string())
    }

    /// 标记引擎启动失败。
    ///
    /// 后续所有 `get()` 调用将返回失败原因，而非"正在启动中"。
    pub fn set_failed(&self, reason: String) {
        if let Ok(mut f) = self.failure.write() {
            *f = Some(reason);
        }
    }

    /// 获取引擎状态引用。
    ///
    /// 返回值：
    /// - `Ok(&AppState)` — 引擎就绪
    /// - `Err("引擎启动失败: ...")` — 引擎启动失败
    /// - `Err("引擎正在启动中，请稍后重试")` — 引擎正在启动
    pub fn get(&self) -> Result<&AppState, String> {
        // 优先检查是否已就绪
        if let Some(state) = self.inner.get() {
            return Ok(state);
        }

        // 检查是否启动失败
        if let Ok(guard) = self.failure.read() {
            if let Some(reason) = guard.as_ref() {
                return Err(format!("引擎启动失败: {}", reason));
            }
        }

        // 仍在启动中
        Err("引擎正在启动中，请稍后重试".to_string())
    }

    /// 引擎是否已就绪。
    pub fn is_ready(&self) -> bool {
        self.inner.get().is_some()
    }

    /// 引擎是否启动失败。
    pub fn is_failed(&self) -> bool {
        self.failure.read().map(|f| f.is_some()).unwrap_or(false)
    }
}
