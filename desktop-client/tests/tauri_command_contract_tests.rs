//! Tauri 命令契约测试
//!
//! # 测试目标
//!
//! 验证"前端 invoke('xxx') 调用的命令名"与"main.rs invoke_handler 注册表"完全一致。
//! 防止出现 "Command xxx not found" 运行时错误。
//!
//! # 测试策略
//!
//! 使用 `tauri::test` 创建真实的 Tauri 测试 app，通过 IPC 调用每个命令，
//! 验证 Tauri 能路由到该命令（不返回 "Command not found"）。
//!
//! 注意：测试只验证命令**可路由**，不验证业务逻辑正确性（那是单元测试的职责）。
//! 命令可能因为缺少 AppState 而返回业务错误，但只要不是 "Command not found"，
//! 就说明注册表是完整的。
//!
//! # 新增命令时的操作
//!
//! 1. 在 `src/lib.rs` 的 `all_tauri_commands!()` 宏中添加命令
//! 2. 在本文件的 `FRONTEND_INVOKED_COMMANDS` 列表中添加命令名
//! 3. `cargo test --test tauri_command_contract_tests` 验证通过

/// 前端通过 invoke('xxx') 调用的所有命令名。
///
/// 维护方式：从前端源码中提取所有 invoke() 调用，保持与前端同步。
/// 可用以下命令自动提取：
///   grep -rh "invoke(['\"]" desktop-client/src-ui/src \
///     | grep -oP "(?<=invoke\(['\"])[^'\"]+" | sort -u
const FRONTEND_INVOKED_COMMANDS: &[&str] = &[
    // ── 聊天 ────────────────────────────────────────────────────
    "send_chat_message",
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
    // ── 扩展管理 ────────────────────────────────────────────────
    "ic_list_extensions",
    "ic_install_extension",
    "ic_uninstall_extension",
    "ic_search_extensions",
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
    // ── 认证 ────────────────────────────────────────────────────
    "get_auth_token",
];

/// 验证所有前端调用的命令都已注册到 invoke_handler。
///
/// 使用 Tauri 测试运行时，通过 IPC 调用每个命令。
/// 如果命令未注册，Tauri 会返回包含 "Command xxx not found" 的错误。
/// 如果命令已注册但因缺少 AppState 等原因失败，则视为通过（命令可路由）。
#[test]
fn test_all_frontend_commands_are_registered() {
    let app = tauri::test::mock_builder()
        .invoke_handler(desktop_client::all_tauri_commands!())
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .expect("Failed to build test app");

    // Tauri v2 需要先创建 WebviewWindow 才能调用 get_ipc_response
    let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("Failed to create test webview");

    let mut unregistered = Vec::new();

    for &cmd_name in FRONTEND_INVOKED_COMMANDS {
        // 通过 IPC 调用命令，传入空参数
        // 命令可能因为缺少参数或 AppState 而返回错误，但不应该是 "Command not found"
        let response = tauri::test::get_ipc_response(
            &webview,
            tauri::webview::InvokeRequest {
                cmd: cmd_name.to_string(),
                callback: tauri::ipc::CallbackFn(0),
                error: tauri::ipc::CallbackFn(1),
                url: "tauri://localhost".parse().unwrap(),
                body: tauri::ipc::InvokeBody::Json(serde_json::json!({})),
                headers: Default::default(),
                invoke_key: tauri::test::INVOKE_KEY.to_string(),
            },
        );

        // 检查是否是 "Command not found" 错误
        if let Err(e) = &response {
            let err_str = format!("{:?}", e);
            if err_str.contains("not found") || err_str.contains("Command") {
                unregistered.push(cmd_name);
            }
            // 其他错误（如缺少参数、AppState 未初始化）是可以接受的
        }
    }

    if !unregistered.is_empty() {
        panic!(
            "\n❌ 以下命令在前端被调用，但未注册到 invoke_handler:\n{}\n\n\
             修复方法：在 desktop-client/src/lib.rs 的 all_tauri_commands!() 宏中添加这些命令。",
            unregistered
                .iter()
                .map(|c| format!("   - {}", c))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    println!(
        "✅ 契约测试通过：{} 个前端命令全部已注册",
        FRONTEND_INVOKED_COMMANDS.len()
    );
}

/// 验证注册表中没有前端从未调用的"幽灵命令"（可选，用于清理）。
///
/// 这不是强制要求，但有助于发现已废弃但未清理的命令。
#[test]
fn test_no_ghost_commands_in_handler() {
    // 从 all_tauri_commands!() 宏提取注册的命令名
    // 由于宏展开后是 Tauri 内部类型，我们用静态列表维护
    // 这个列表必须与 lib.rs 的 all_tauri_commands!() 保持同步
    const REGISTERED_COMMANDS: &[&str] = &[
        "send_chat_message",
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
        "ic_list_extensions",
        "ic_install_extension",
        "ic_uninstall_extension",
        "ic_search_extensions",
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
        "get_auth_token",
    ];

    let frontend_set: std::collections::HashSet<&str> =
        FRONTEND_INVOKED_COMMANDS.iter().copied().collect();

    let ghost_commands: Vec<&str> = REGISTERED_COMMANDS
        .iter()
        .copied()
        .filter(|cmd| !frontend_set.contains(cmd))
        .collect();

    if !ghost_commands.is_empty() {
        // 只打印警告，不 panic — 幽灵命令不是错误，只是技术债
        println!(
            "⚠️  以下命令已注册但前端未调用（可能是废弃命令）:\n{}",
            ghost_commands
                .iter()
                .map(|c| format!("   - {}", c))
                .collect::<Vec<_>>()
                .join("\n")
        );
    } else {
        println!("✅ 注册表整洁：无幽灵命令");
    }
}
