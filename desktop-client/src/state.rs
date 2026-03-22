//! Tauri 全局状态 — 持有 IronClaw 组件的引用。
//!
//! `AppState` 在引擎启动后通过 `app_handle.manage()` 注入 Tauri，
//! 所有 Tauri Command 通过 `tauri::State<'_, AppState>` 访问 IronClaw 组件。

use std::sync::Arc;

use ironclaw::channels::IncomingMessage;
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

/// Tauri 全局状态。
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
    /// 安全层（DLP / 内容过滤）。
    pub safety: Arc<SafetyLayer>,
    /// 安全桥接器（统一 SafetyLayer + DLP 格式保留脱敏）。
    pub safety_bridge: Arc<SafetyBridge>,
    /// 上下文管理器（任务审批等）。
    pub context_manager: Arc<ContextManager>,
    /// 实例 owner ID。
    pub owner_id: String,
}
