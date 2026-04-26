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

use std::collections::HashSet;
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
use ironclaw::tools::builtin::SchedulerSlot;
use ironclaw::tools::ToolRegistry;
use ironclaw::workspace::Workspace;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::conversation_tracker::ConversationTracker;
use crate::data_reporter::DataReporter;
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
    /// 对话追踪器（对话审计与技能使用上报）。
    pub conversation_tracker: Arc<ConversationTracker>,
    /// 数据上报器（对话审计/DLP/运行指标出站）。
    pub data_reporter: Arc<DataReporter>,
    /// IronClaw 本地租户/作用域 ID。
    pub scope_id: String,
    /// Admin Backend 中当前客户端绑定的真实用户 ID。
    pub backend_user_id: Arc<std::sync::RwLock<Option<Uuid>>>,
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
    /// Scheduler slot — Agent 构建完成后填充，供任务取消/重试命令使用。
    pub scheduler_slot: SchedulerSlot,
    /// 被用户禁用的技能名集合（仅影响 desktop-client IPC 行为）。
    pub disabled_skills: std::sync::RwLock<HashSet<String>>,
    /// 被用户禁用的扩展名集合（仅影响 desktop-client IPC 行为）。
    pub disabled_extensions: std::sync::RwLock<HashSet<String>>,
}

impl AppState {
    pub fn require_backend_user_id(&self, context: &str) -> Result<Uuid, String> {
        require_backend_user_id_value(&self.backend_user_id, context)
    }

    pub fn skill_enabled(&self, name: &str) -> bool {
        self.disabled_skills
            .read()
            .map(|set| !set.contains(name))
            .unwrap_or(true)
    }

    pub fn extension_enabled(&self, name: &str) -> bool {
        self.disabled_extensions
            .read()
            .map(|set| !set.contains(name))
            .unwrap_or(true)
    }

    pub fn set_skill_enabled(&self, name: &str, enabled: bool) -> Result<(), String> {
        set_enabled_flag(&self.disabled_skills, name, enabled)
    }

    pub fn set_extension_enabled(&self, name: &str, enabled: bool) -> Result<(), String> {
        set_enabled_flag(&self.disabled_extensions, name, enabled)
    }

    pub fn disabled_skills_snapshot(&self) -> Result<Vec<String>, String> {
        disabled_snapshot(&self.disabled_skills)
    }

    pub fn disabled_extensions_snapshot(&self) -> Result<Vec<String>, String> {
        disabled_snapshot(&self.disabled_extensions)
    }
}

pub(crate) fn require_backend_user_id_value(
    backend_user_id: &std::sync::RwLock<Option<Uuid>>,
    context: &str,
) -> Result<Uuid, String> {
    backend_user_id
        .read()
        .map_err(|_| format!("{}：后台用户身份读取失败", context))?
        .to_owned()
        .ok_or_else(|| format!("{}：后台用户身份尚未就绪", context))
}

fn set_enabled_flag(
    lock: &std::sync::RwLock<HashSet<String>>,
    name: &str,
    enabled: bool,
) -> Result<(), String> {
    let mut guard = lock.write().map_err(|e| format!("Lock poisoned: {}", e))?;
    if enabled {
        guard.remove(name);
        return Ok(());
    }
    guard.insert(name.to_string());
    Ok(())
}

fn disabled_snapshot(lock: &std::sync::RwLock<HashSet<String>>) -> Result<Vec<String>, String> {
    let guard = lock.read().map_err(|e| format!("Lock poisoned: {}", e))?;
    Ok(guard.iter().cloned().collect())
}

#[cfg(test)]
mod tests {
    use super::{disabled_snapshot, require_backend_user_id_value, set_enabled_flag};
    use std::collections::HashSet;
    use std::sync::RwLock;
    use uuid::Uuid;

    #[test]
    fn test_set_enabled_flag_disable_then_enable() {
        let lock = RwLock::new(HashSet::new());

        set_enabled_flag(&lock, "github", false).expect("disable should succeed");
        let disabled = disabled_snapshot(&lock).expect("snapshot should succeed");
        assert_eq!(disabled, vec!["github".to_string()]);

        set_enabled_flag(&lock, "github", true).expect("enable should succeed");
        let disabled = disabled_snapshot(&lock).expect("snapshot should succeed");
        assert!(disabled.is_empty(), "enabled extension should be removed");
    }

    #[test]
    fn test_set_enabled_flag_idempotent_disable() {
        let lock = RwLock::new(HashSet::new());

        set_enabled_flag(&lock, "calendar", false).expect("first disable should succeed");
        set_enabled_flag(&lock, "calendar", false).expect("second disable should succeed");

        let disabled = disabled_snapshot(&lock).expect("snapshot should succeed");
        assert_eq!(disabled, vec!["calendar".to_string()]);
    }

    #[test]
    fn test_disabled_snapshot_returns_all_disabled_names() {
        let lock = RwLock::new(HashSet::new());
        set_enabled_flag(&lock, "a", false).expect("disable a should succeed");
        set_enabled_flag(&lock, "b", false).expect("disable b should succeed");

        let mut disabled = disabled_snapshot(&lock).expect("snapshot should succeed");
        disabled.sort();
        assert_eq!(disabled, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn test_require_backend_user_id_value_returns_uuid() {
        let expected =
            Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").expect("uuid should parse");
        let lock = RwLock::new(Some(expected));

        let actual = require_backend_user_id_value(&lock, "配额预检")
            .expect("backend user id should be available");

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_require_backend_user_id_value_reports_missing_identity() {
        let lock = RwLock::new(None);

        let error = require_backend_user_id_value(&lock, "审批申请人身份")
            .expect_err("missing backend user id should fail");

        assert_eq!(error, "审批申请人身份：后台用户身份尚未就绪");
    }

    #[test]
    fn test_require_backend_user_id_value_reports_poisoned_lock() {
        let lock = RwLock::new(Some(Uuid::nil()));
        let _ = std::panic::catch_unwind(|| {
            let _guard = lock.write().expect("write lock should succeed");
            panic!("poison backend user id lock");
        });

        let error = require_backend_user_id_value(&lock, "配额预检")
            .expect_err("poisoned backend user id lock should fail");

        assert_eq!(error, "配额预检：后台用户身份读取失败");
    }
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
