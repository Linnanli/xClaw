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
            setup_master_password,
            unlock_app,
            get_session_info,
            lock_app,
            update_session_activity,
            store_config,
            get_config,
            log_audit_event,
            get_audit_logs,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
