#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use desktop_client::{CommandState, DesktopClientConfig};
use desktop_client::commands::*;
use desktop_client::{
    AuthTokenManager, AppConfig, NetworkConfig, EnvironmentChecker, platform_utils,
};
use config::{Config, Environment, File};
use std::env;

#[tokio::main]
async fn main() {
    // 第零步：使用 config-rs 加载环境变量文件
    println!("📦 Loading configuration...");
    
    // 检测环境
    let environment = env::var("ENVIRONMENT")
        .unwrap_or_else(|_| {
            // 根据启动命令检测环境
            let args: Vec<String> = env::args().collect();
            if args.iter().any(|arg| arg == "test") {
                "testing".to_string()
            } else if args.iter().any(|arg| arg == "--release") {
                "production".to_string()
            } else {
                "development".to_string()
            }
        });
    
    println!("🔧 Environment: {}", environment);
    
    // 使用 config-rs 加载配置
    let config_builder = Config::builder()
        // 加载默认配置
        .add_source(File::with_name("desktop-client/.env").required(false))
        // 加载环境特定的配置
        .add_source(File::with_name(&format!("desktop-client/.env.{}", environment)).required(false))
        // 加载系统环境变量
        .add_source(Environment::default().try_parsing(true).separator("_"));
    
    match config_builder.build() {
        Ok(config) => {
            // 将配置加载到环境变量
            if let Ok(settings) = config.try_deserialize::<std::collections::HashMap<String, String>>() {
                for (key, value) in settings {
                    env::set_var(&key, &value);
                }
            }
            println!("✅ Configuration loaded");
        }
        Err(e) => {
            eprintln!("⚠️  Warning: Failed to load configuration: {}", e);
        }
    }
    
    // 设置 ENVIRONMENT 环境变量
    env::set_var("ENVIRONMENT", &environment);
    println!();
    
    // 第一步：检查环境一致性
    println!("🔍 Checking environment consistency...");
    let mut checker = EnvironmentChecker::new();
    if !checker.run_all_checks() {
        eprintln!("❌ Environment check failed!");
        checker.print_results();
        std::process::exit(1);
    }
    println!("✅ Environment check passed");
    
    // 第二步：初始化认证令牌
    println!("🔐 Initializing authentication token...");
    let token_manager = AuthTokenManager::new();
    let auth_token = match token_manager.load_or_generate() {
        Ok(token) => {
            println!("✅ Auth token initialized: {}", &token[..8]);
            token
        }
        Err(e) => {
            eprintln!("❌ Failed to initialize auth token: {}", e);
            std::process::exit(1);
        }
    };
    
    // 第三步：加载应用配置
    println!("⚙️  Loading application configuration...");
    let app_config = AppConfig::load_or_default();
    if let Err(e) = app_config.validate() {
        eprintln!("❌ Invalid configuration: {}", e);
        std::process::exit(1);
    }
    println!("✅ Configuration loaded: {}", app_config.api_base_url);
    
    // 第四步：加载网络配置
    println!("🌐 Loading network configuration...");
    let network_config = NetworkConfig::from_env();
    if let Err(e) = network_config.validate() {
        eprintln!("❌ Invalid network configuration: {}", e);
        std::process::exit(1);
    }
    println!("✅ Network configuration loaded");
    
    // 第五步：确保数据目录存在
    println!("📁 Ensuring data directories exist...");
    let config = DesktopClientConfig::default();
    if !config.data_dir.exists() {
        std::fs::create_dir_all(&config.data_dir).expect("Failed to create data directory");
    }
    
    // 确保其他必要的目录存在
    let _ = platform_utils::create_dir_if_not_exists(&platform_utils::get_config_dir());
    let _ = platform_utils::create_dir_if_not_exists(&platform_utils::get_cache_dir());
    println!("✅ Data directories ready");
    
    // 第六步：打印启动信息
    println!("\n🚀 Starting Ironclaw Desktop Client");
    println!("   Environment: {:?}", checker.get_config().environment);
    println!("   API URL: {}", app_config.api_base_url);
    println!("   Database: {}", app_config.database_type);
    println!("   OS: {}", platform_utils::get_os_name());
    println!("   Log Level: {}\n", app_config.log_level);

    let state = CommandState::new_with_token(auth_token);

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
            // 环境和配置管理命令
            get_app_init_info,
            get_auth_token,
            refresh_auth_token,
            get_app_config,
            get_network_config,
            check_environment_consistency,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
