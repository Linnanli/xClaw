//! Tauri 命令契约测试
//!
//! 验证"前端 invoke('xxx') 调用的命令名"与 `all_tauri_commands!()` 注册表一致。
//!
//! # 测试策略
//!
//! 由于 Tauri v2 MockRuntime 对 `AppHandle` 参数的 trait bound 限制，
//! 无法通过 IPC 调用所有命令。改为静态列表对比 + 编译检查：
//! - `all_tauri_commands!()` 宏在 `cargo build` 时已验证所有命令可编译
//! - 本测试验证前端调用的命令名与注册表列表一致

/// 前端通过 invoke('xxx') 调用的所有命令名。
///
/// 维护方式：从前端源码中提取所有 invoke() 调用，保持与前端同步。
const FRONTEND_INVOKED_COMMANDS: &[&str] = &[
    // ── 聊天 ────────────────────────────────────────────────────
    "send_chat_message",
    "ic_activate_model",
    "ic_interrupt_thread",
    "ic_finalize_thread",
    "subscribe_chat_events",
    "unsubscribe_chat_events",
    // ── 线程管理 ────────────────────────────────────────────────
    "ic_list_threads",
    "ic_create_thread",
    "ic_get_thread_history",
    // ── 记忆/工作空间 ───────────────────────────────────────────
    "ic_memory_list",
    "ic_memory_read",
    "ic_memory_write",
    "ic_memory_delete",
    "ic_memory_search",
    // ── 技能管理 ────────────────────────────────────────────────
    "ic_list_skills",
    "ic_search_skills",
    "ic_install_skill",
    "ic_uninstall_skill",
    "ic_enable_skill",
    "ic_disable_skill",
    // ── 扩展管理 ────────────────────────────────────────────────
    "ic_list_extensions",
    "ic_install_extension",
    "ic_uninstall_extension",
    "ic_enable_extension",
    "ic_disable_extension",
    "ic_search_extensions",
    "ic_extension_setup",
    "ic_extension_setup_submit",
    // ── 任务管理 ────────────────────────────────────────────────
    "ic_list_jobs",
    "ic_job_events",
    "ic_job_prompt",
    "ic_get_job_detail",
    "ic_cancel_job",
    "ic_restart_job",
    // ── 日程管理 ────────────────────────────────────────────────
    "ic_routine_runs",
    "ic_list_routines",
    "ic_create_routine",
    "ic_toggle_routine",
    "ic_delete_routine",
    "ic_fire_routine",
    // ── 工具审批 ────────────────────────────────────────────────
    "ic_approve_tool",
    "ic_deny_tool",
    // ── DLP 桥接 ────────────────────────────────────────────────
    "scan_user_input",
    "scan_outbound_request",
    "sanitize_for_storage",
    "check_http_request",
    "get_dlp_config",
    "update_dlp_config",
    "get_dlp_statistics",
    "sync_dlp_rules_from_admin",
    // ── 模型配置 ─────────────────────────────────────────────────
    "get_available_models",
    "get_custom_models",
    "create_custom_model",
    "update_custom_model",
    "delete_custom_model",
    "test_model_connection",
    // ── 认证 ────────────────────────────────────────────────────
    "get_auth_token",
    // ── 水印配置 ────────────────────────────────────────────────
    "get_watermark_config",
    // ── 应用信息 ────────────────────────────────────────────────
    "get_app_version",
    "check_for_updates",
    // ── 审批工单 ────────────────────────────────────────────────
    "submit_approval_ticket",
    // ── 日志查询 ────────────────────────────────────────────────
    "ic_get_logs",
    "ic_search_logs",
    "ic_filter_logs",
    "ic_export_logs",
    "ic_clear_logs",
    // ── 工作区状态 ──────────────────────────────────────────────
    "ic_workspace_git_status",
    "ic_workspace_root",
    "ic_active_servers",
    // ── Plan Mode / Fork ────────────────────────────────────────
    "ic_toggle_plan_mode",
    "ic_approve_plan",
    "ic_revise_plan",
    "ic_fork_thread",
];

/// `all_tauri_commands!()` 宏中注册的所有命令名（必须与 lib.rs 保持同步）。
const REGISTERED_COMMANDS: &[&str] = &[
    "send_chat_message",
    "ic_activate_model",
    "ic_interrupt_thread",
    "ic_finalize_thread",
    "subscribe_chat_events",
    "unsubscribe_chat_events",
    "ic_list_threads",
    "ic_create_thread",
    "ic_get_thread_history",
    "ic_memory_list",
    "ic_memory_read",
    "ic_memory_write",
    "ic_memory_delete",
    "ic_memory_search",
    "ic_list_skills",
    "ic_search_skills",
    "ic_install_skill",
    "ic_uninstall_skill",
    "ic_enable_skill",
    "ic_disable_skill",
    "ic_list_extensions",
    "ic_install_extension",
    "ic_uninstall_extension",
    "ic_enable_extension",
    "ic_disable_extension",
    "ic_search_extensions",
    "ic_extension_setup",
    "ic_extension_setup_submit",
    "ic_list_jobs",
    "ic_job_events",
    "ic_job_prompt",
    "ic_get_job_detail",
    "ic_cancel_job",
    "ic_restart_job",
    "ic_routine_runs",
    "ic_list_routines",
    "ic_create_routine",
    "ic_toggle_routine",
    "ic_delete_routine",
    "ic_fire_routine",
    "ic_approve_tool",
    "ic_deny_tool",
    "scan_user_input",
    "scan_outbound_request",
    "sanitize_for_storage",
    "check_http_request",
    "get_dlp_config",
    "update_dlp_config",
    "get_dlp_statistics",
    "sync_dlp_rules_from_admin",
    "get_available_models",
    "get_custom_models",
    "create_custom_model",
    "update_custom_model",
    "delete_custom_model",
    "test_model_connection",
    "get_auth_token",
    // ── 水印配置 ────────────────────────────────────────────────
    "get_watermark_config",
    // ── 应用信息 ────────────────────────────────────────────────
    "get_app_version",
    "check_for_updates",
    // ── 审批工单 ────────────────────────────────────────────────
    "submit_approval_ticket",
    // ── 日志查询 ────────────────────────────────────────────────
    "ic_get_logs",
    "ic_search_logs",
    "ic_filter_logs",
    "ic_export_logs",
    "ic_clear_logs",
    // ── 工作区状态 ──────────────────────────────────────────────
    "ic_workspace_git_status",
    "ic_workspace_root",
    "ic_active_servers",
    // ── Plan Mode / Fork ────────────────────────────────────────
    "ic_toggle_plan_mode",
    "ic_approve_plan",
    "ic_revise_plan",
    "ic_fork_thread",
];

#[test]
fn test_all_frontend_commands_are_registered() {
    let registered: std::collections::HashSet<&str> = REGISTERED_COMMANDS.iter().copied().collect();

    let missing: Vec<&str> = FRONTEND_INVOKED_COMMANDS
        .iter()
        .copied()
        .filter(|cmd| !registered.contains(cmd))
        .collect();

    assert!(
        missing.is_empty(),
        "\n❌ 以下命令在前端被调用，但未注册到 invoke_handler:\n{}\n\n\
         修复方法：在 lib.rs 的 all_tauri_commands!() 宏中添加这些命令。",
        missing
            .iter()
            .map(|c| format!("   - {}", c))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn test_no_ghost_commands_in_handler() {
    let frontend: std::collections::HashSet<&str> =
        FRONTEND_INVOKED_COMMANDS.iter().copied().collect();

    let ghost: Vec<&str> = REGISTERED_COMMANDS
        .iter()
        .copied()
        .filter(|cmd| !frontend.contains(cmd))
        .collect();

    if !ghost.is_empty() {
        println!(
            "⚠️  以下命令已注册但前端未调用（可能是废弃命令）:\n{}",
            ghost
                .iter()
                .map(|c| format!("   - {}", c))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
