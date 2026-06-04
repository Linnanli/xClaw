//! 工作区状态 Tauri Commands。
//!
//! 提供 Git 状态、工作区根目录、活跃服务器列表等信息，
//! 以及工作区导入/查询功能，用于桌面客户端侧边栏。

use serde::Serialize;
use tauri::State;
use uuid::Uuid;

use crate::state::{AppState, EngineState};

/// 活跃的 LSP/MCP 服务器。
#[derive(Debug, Clone, Serialize)]
pub struct ActiveServer {
    pub name: String,
    #[serde(rename = "type")]
    pub server_type: String,
}

/// 获取当前工作目录的 git status 输出。
#[tauri::command]
pub async fn ic_workspace_git_status(state: State<'_, EngineState>) -> Result<String, String> {
    let _state = state.get()?;

    let output = tokio::process::Command::new("git")
        .args(["status"])
        .output()
        .await
        .map_err(|e| format!("Failed to run git: {e}"))?;

    git_status_result(output.status.success(), &output.stdout, &output.stderr)
}

pub(crate) fn git_status_result(
    success: bool,
    stdout: &[u8],
    stderr: &[u8],
) -> Result<String, String> {
    if !success {
        let stderr = String::from_utf8_lossy(stderr);
        return Err(format!("git status failed: {stderr}"));
    }

    Ok(String::from_utf8_lossy(stdout).into_owned())
}

/// 返回当前工作目录路径。
#[tauri::command]
pub async fn ic_workspace_root(state: State<'_, EngineState>) -> Result<String, String> {
    let _state = state.get()?;

    std::env::current_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|e| format!("Cannot determine working directory: {e}"))
}

/// 列出当前活跃的 LSP 和 MCP 服务器。
#[tauri::command]
pub async fn ic_active_servers(state: State<'_, EngineState>) -> Result<Vec<ActiveServer>, String> {
    let state = state.get()?;
    let mut servers = Vec::new();

    // MCP servers from extension manager
    if let Some(ref ext_mgr) = state.extension_manager {
        if let Ok(extensions) = ext_mgr
            .list(
                Some(ironclaw::extensions::ExtensionKind::McpServer),
                false,
                &state.scope_id,
            )
            .await
        {
            for ext in extensions {
                if ext.active {
                    servers.push(ActiveServer {
                        name: ext.name,
                        server_type: "mcp".to_string(),
                    });
                }
            }
        }
    }

    Ok(servers)
}

/// 为指定线程导入用户工作区。
///
/// 验证路径有效性后将其写入对话 metadata 的 `workspace_root` 字段，
/// 覆盖自动创建的沙箱路径。
#[tauri::command]
pub async fn ic_import_workspace(
    state: State<'_, EngineState>,
    thread_id: String,
    path: String,
) -> Result<String, String> {
    import_workspace_from_state(state.get()?, &thread_id, &path).await
}

pub(crate) async fn import_workspace_from_state(
    state: &AppState,
    thread_id: &str,
    path: &str,
) -> Result<String, String> {
    let db = state.db.as_ref().ok_or("Database not available")?;

    let tid: Uuid = thread_id
        .parse()
        .map_err(|_| format!("Invalid thread_id: {thread_id}"))?;

    let canonical = crate::workspace_dir::validate_import_path(path)
        .map_err(|e| format!("Invalid workspace path: {e}"))?;
    let canonical_str = canonical.to_string_lossy().to_string();

    db.update_conversation_metadata_field(tid, "workspace_root", &serde_json::json!(canonical_str))
        .await
        .map_err(|e| format!("Failed to update workspace: {e}"))?;

    tracing::info!(thread_id = %tid, workspace = %canonical_str, "Workspace imported");
    Ok(canonical_str)
}

/// 查询指定线程的工作区路径。
#[tauri::command]
pub async fn ic_get_thread_workspace(
    state: State<'_, EngineState>,
    thread_id: String,
) -> Result<Option<String>, String> {
    get_thread_workspace_from_state(state.get()?, &thread_id).await
}

pub(crate) async fn get_thread_workspace_from_state(
    state: &AppState,
    thread_id: &str,
) -> Result<Option<String>, String> {
    let db = state.db.as_ref().ok_or("Database not available")?;

    let tid: Uuid = thread_id
        .parse()
        .map_err(|_| format!("Invalid thread_id: {thread_id}"))?;

    let meta = db
        .get_conversation_metadata(tid)
        .await
        .map_err(|e| format!("Failed to read metadata: {e}"))?;

    Ok(meta.and_then(|m| {
        m.get("workspace_root")
            .and_then(|v| v.as_str().map(String::from))
    }))
}

/// 列出 `~/.ironclaw/projects/` 下所有沙箱工作区。
#[tauri::command]
pub async fn ic_list_sandbox_workspaces() -> Result<Vec<SandboxWorkspace>, String> {
    let base = crate::workspace_dir::projects_base();
    if !tokio::fs::try_exists(&base).await.unwrap_or(false) {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    let mut dir = tokio::fs::read_dir(&base)
        .await
        .map_err(|e| format!("Failed to read projects directory: {e}"))?;

    while let Some(entry) = dir
        .next_entry()
        .await
        .map_err(|e: std::io::Error| e.to_string())?
    {
        let ft = entry
            .file_type()
            .await
            .map_err(|e: std::io::Error| e.to_string())?;
        if ft.is_dir() {
            entries.push(SandboxWorkspace {
                name: entry.file_name().to_string_lossy().into_owned(),
                path: entry.path().to_string_lossy().into_owned(),
            });
        }
    }

    Ok(entries)
}

/// 沙箱工作区简要信息。
#[derive(Debug, Clone, Serialize)]
pub struct SandboxWorkspace {
    pub name: String,
    pub path: String,
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::sync::Arc;

    use dasclaw_runtime::context::ContextManager;
    use tokio::sync::mpsc;

    use super::{
        get_thread_workspace_from_state, git_status_result, import_workspace_from_state,
        ActiveServer, SandboxWorkspace,
    };
    use crate::state::AppState;

    struct StubLlmProvider;

    #[async_trait::async_trait]
    impl ironclaw::llm::LlmProvider for StubLlmProvider {
        fn model_name(&self) -> &str {
            "stub-model"
        }

        fn cost_per_token(&self) -> (rust_decimal::Decimal, rust_decimal::Decimal) {
            (rust_decimal::Decimal::ZERO, rust_decimal::Decimal::ZERO)
        }

        async fn complete(
            &self,
            _req: ironclaw::llm::CompletionRequest,
        ) -> Result<ironclaw::llm::CompletionResponse, ironclaw::error::LlmError> {
            Err(ironclaw::error::LlmError::RequestFailed {
                provider: "stub".into(),
                reason: "not implemented".into(),
            })
        }

        async fn complete_with_tools(
            &self,
            _req: ironclaw::llm::ToolCompletionRequest,
        ) -> Result<ironclaw::llm::ToolCompletionResponse, ironclaw::error::LlmError> {
            Err(ironclaw::error::LlmError::RequestFailed {
                provider: "stub".into(),
                reason: "not implemented".into(),
            })
        }
    }

    async fn test_db(tempdir: &tempfile::TempDir) -> Arc<dyn ironclaw::db::Database> {
        let db_path = tempdir.path().join("workspace-state.db");
        let backend = ironclaw::db::libsql::LibSqlBackend::new_local(&db_path)
            .await
            .expect("file-backed libsql backend should initialize");
        <ironclaw::db::libsql::LibSqlBackend as ironclaw::db::Database>::run_migrations(&backend)
            .await
            .expect("migrations should run");
        Arc::new(backend)
    }

    async fn test_app_state_with_db() -> (AppState, tempfile::TempDir) {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        let db = test_db(&tempdir).await;
        let (tx, _rx) = mpsc::channel(1);

        let safety_config = ironclaw::safety::SafetyConfig {
            max_output_length: 100_000,
            injection_check_enabled: true,
        };
        let safety = Arc::new(ironclaw::safety::SafetyLayer::new(&safety_config));
        let safety_bridge = Arc::new(crate::safety_bridge::SafetyBridge::new(
            Arc::clone(&safety),
            None,
            None,
        ));
        let egress: Arc<dyn dasclaw_core::EgressGate> = Arc::new(
            dasclaw_safety::egress_gate::IronclawEgressGate::new(Arc::clone(&safety)),
        );
        let attachment_scanner = Arc::new(
            crate::safety_attachment_scanner::AttachmentScanner::new(Arc::clone(&egress)),
        );
        let model_override = Arc::new(std::sync::RwLock::new(None));
        let stub_llm: Arc<dyn ironclaw::llm::LlmProvider> = Arc::new(StubLlmProvider);
        let model_switch = Arc::new(crate::model_switch::ModelSwitchProvider::new(
            Arc::clone(&stub_llm),
            Arc::clone(&model_override),
        ));

        let state = AppState {
            msg_sender: tx,
            db: Some(db),
            workspace: None,
            tools: Arc::new(ironclaw::tools::ToolRegistry::new()),
            extension_manager: None,
            skill_registry: None,
            skill_catalog: None,
            skills_config: ironclaw::config::SkillsConfig::default(),
            safety,
            safety_bridge,
            attachment_scanner,
            egress,
            context_manager: Arc::new(ContextManager::new(5)),
            conversation_tracker: Arc::new(crate::conversation_tracker::ConversationTracker::new(
                "workspace-ipc-owner".to_string(),
            )),
            data_reporter: Arc::new(crate::data_reporter::DataReporter::new_for_test()),
            scope_id: "workspace-ipc-owner".to_string(),
            backend_user_id: Arc::new(std::sync::RwLock::new(None)),
            llm: Arc::clone(&model_switch) as _,
            model_override,
            model_switch,
            provider_base_url: std::sync::RwLock::new(String::new()),
            initial_provider: Arc::clone(&stub_llm),
            initial_base_url: String::new(),
            log_broadcaster: Arc::new(ironclaw::channels::web::log_layer::LogBroadcaster::new()),
            log_clear_offset: std::sync::atomic::AtomicUsize::new(0),
            routine_engine_slot: Arc::new(tokio::sync::RwLock::new(None)),
            scheduler_slot: Arc::new(tokio::sync::RwLock::new(None)),
            disabled_skills: std::sync::RwLock::new(HashSet::new()),
            disabled_extensions: std::sync::RwLock::new(HashSet::new()),
        };

        (state, tempdir)
    }

    #[test]
    fn req_workspace_git_status_returns_stdout_on_success() {
        let stdout = b"On branch xClaw\nnothing to commit, working tree clean\n";

        let result = git_status_result(true, stdout, b"").expect("status should succeed");

        assert!(result.contains("On branch xClaw"));
        assert!(result.contains("working tree clean"));
    }

    #[test]
    fn test_workspace_failure_git_status_surfaces_stderr() {
        let err = git_status_result(false, b"", b"fatal: not a git repository")
            .expect_err("status should fail");

        assert!(err.contains("git status failed"));
        assert!(err.contains("not a git repository"));
    }

    #[test]
    fn req_workspace_active_server_contract_renames_type_field() {
        let server = ActiveServer {
            name: "filesystem".to_string(),
            server_type: "mcp".to_string(),
        };

        let json = serde_json::to_value(&server).expect("serialize active server");

        assert_eq!(json["name"], "filesystem");
        assert_eq!(json["type"], "mcp");
        assert!(json.get("server_type").is_none());
    }

    #[test]
    fn req_workspace_sandbox_workspace_contract_matches_frontend() {
        let workspace = SandboxWorkspace {
            name: "thread-123".to_string(),
            path: "/tmp/thread-123".to_string(),
        };

        let json = serde_json::to_value(&workspace).expect("serialize workspace");

        assert_eq!(json["name"], "thread-123");
        assert_eq!(json["path"], "/tmp/thread-123");
    }

    #[tokio::test]
    async fn req_workspace_import_and_get_thread_workspace_state_round_trip() {
        let (state, tempdir) = test_app_state_with_db().await;
        let import_dir = tempdir.path().join("imported-workspace");
        std::fs::create_dir_all(&import_dir).expect("import workspace dir should be created");

        let db = state.db.as_ref().expect("db should exist");
        let thread_id = db
            .create_conversation_with_metadata(
                "tauri",
                &state.scope_id,
                &serde_json::json!({"workspace_root": "/old/sandbox/path", "title": "Workspace"}),
            )
            .await
            .expect("conversation should be created");
        let thread_id_str = thread_id.to_string();

        let imported =
            import_workspace_from_state(&state, &thread_id_str, &import_dir.to_string_lossy())
                .await
                .expect("import should persist canonical workspace path");
        let expected = import_dir
            .canonicalize()
            .expect("import dir should canonicalize")
            .to_string_lossy()
            .to_string();
        assert_eq!(imported, expected);

        let loaded = get_thread_workspace_from_state(&state, &thread_id_str)
            .await
            .expect("get should read metadata from real db");
        assert_eq!(loaded.as_deref(), Some(expected.as_str()));

        let metadata = db
            .get_conversation_metadata(thread_id)
            .await
            .expect("metadata should be readable")
            .expect("metadata should exist");
        assert_eq!(metadata["workspace_root"], serde_json::json!(expected));
        assert_eq!(
            metadata["title"],
            serde_json::json!("Workspace"),
            "import should patch workspace_root without dropping existing metadata"
        );
    }

    #[tokio::test]
    async fn test_workspace_import_failure_rejects_non_directory_without_metadata_write() {
        let (state, tempdir) = test_app_state_with_db().await;
        let file_path = tempdir.path().join("not-a-directory.txt");
        std::fs::write(&file_path, "not a directory").expect("test file should be written");

        let db = state.db.as_ref().expect("db should exist");
        let thread_id = db
            .create_conversation_with_metadata(
                "tauri",
                &state.scope_id,
                &serde_json::json!({"workspace_root": "/old/sandbox/path"}),
            )
            .await
            .expect("conversation should be created");
        let thread_id = thread_id.to_string();

        let error = import_workspace_from_state(&state, &thread_id, &file_path.to_string_lossy())
            .await
            .expect_err("import should reject file paths");
        assert!(
            error.contains("Invalid workspace path"),
            "error should come from path validation, actual: {error}"
        );

        let loaded = get_thread_workspace_from_state(&state, &thread_id)
            .await
            .expect("get should still read old metadata");
        assert_eq!(loaded.as_deref(), Some("/old/sandbox/path"));
    }
}
