#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use desktop_client::{CommandState, DesktopClientConfig};
use desktop_client::commands::*;
use std::path::PathBuf;

#[tokio::main]
async fn main() {
    let config = DesktopClientConfig::default();

    // Ensure data directory exists
    if !config.data_dir.exists() {
        std::fs::create_dir_all(&config.data_dir).expect("Failed to create data directory");
    }

    let state = CommandState::new();

    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            check_setup_status,
            setup_master_password,
            unlock_app,
            get_session_info,
            lock_app,
            update_session_activity,
            store_config,
            get_config,
            log_audit_event,
            get_audit_logs,
            get_installed_plugins,
            get_available_plugins,
            check_plugin_updates,
            install_plugin,
            uninstall_plugin,
            enable_plugin,
            disable_plugin,
            update_plugin,
            get_offline_state,
            enable_offline_mode,
            disable_offline_mode,
            get_offline_capabilities,
            can_perform_operation,
            get_threads,
            create_thread,
            send_message,
            approve_operation,
            deny_operation,
            get_installed_extensions,
            get_available_extensions,
            install_extension,
            uninstall_extension,
            enable_extension,
            disable_extension,
            search_extensions,
            get_enabled_tools,
            get_routines,
            create_routine,
            delete_routine,
            trigger_routine,
            enable_routine,
            disable_routine,
            pause_routine,
            get_routine_runs,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
