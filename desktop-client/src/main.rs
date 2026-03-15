#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use desktop_client::{CommandState, DesktopClientConfig};
use desktop_client::commands::*;

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
            get_messages,
            search_messages,
            edit_message,
            delete_message,
            export_thread,
            upload_file,
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
            get_available_skills,
            get_installed_skills,
            install_skill,
            uninstall_skill,
            enable_skill,
            disable_skill,
            get_routines,
            create_routine,
            delete_routine,
            trigger_routine,
            enable_routine,
            disable_routine,
            pause_routine,
            get_routine_runs,
            get_memory_tree,
            read_memory,
            write_memory,
            search_memory,
            get_jobs,
            get_job_detail,
            cancel_job,
            restart_job,
            get_logs,
            search_logs,
            filter_logs,
            export_logs,
            clear_logs,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
