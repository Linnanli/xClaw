#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use desktop_client::{CommandState, DesktopClientConfig};
use desktop_client::commands::*;
use desktop_client::{
    AuthTokenManager, AppConfig, NetworkConfig, EnvironmentChecker, platform_utils,
};
use config::{Config, Environment, File};
use std::env;
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() {
    // 第零步：使用 config-rs 加载环境变量文件
    println!("📦 Loading configuration...");
    
    // 检测环境 (优先级: 环境变量 > 编译配置 > 默认值)
    let environment = env::var("ENVIRONMENT").unwrap_or_else(|_| {
        if cfg!(debug_assertions) {
            "development".to_string()
        } else {
            "production".to_string()
        }
    });
    
    println!("🔧 Environment: {}", environment);
    
    // 使用 config-rs 加载配置
    let config_result = Config::builder()
        // 1. 加载默认配置文件
        .add_source(File::with_name("desktop-client/.env").required(false))
        // 2. 加载环境特定的配置文件 (覆盖默认配置)
        .add_source(File::with_name(&format!("desktop-client/.env.{}", environment)).required(false))
        // 3. 加载系统环境变量 (最高优先级)
        .add_source(Environment::default().try_parsing(true).separator("_"))
        .build();
    
    match config_result {
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
            eprintln!("   Continuing with default settings...");
        }
    }
    
    // 确保 ENVIRONMENT 环境变量已设置
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
    
    // 第六步：检查外部 IronClaw 服务器
    // 注意：Desktop Client 使用外部 IronClaw 实例，不内嵌服务器
    // 请确保 IronClaw 服务器已经在运行（端口 38080）
    println!("🔍 Checking external IronClaw server...");
    match desktop_client::embedded_server::check_server_health().await {
        Ok(()) => {
            println!("✅ External IronClaw server is running on port {}", 
                desktop_client::embedded_server::EMBEDDED_SERVER_PORT);
        }
        Err(_) => {
            desktop_client::embedded_server::print_server_instructions();
        }
    }
    
    // 第七步：打印启动信息
    println!("\n🚀 Starting Ironclaw Desktop Client");
    println!("   Environment: {:?}", checker.get_config().environment);
    println!("   API URL: {}", desktop_client::embedded_server::get_server_url());
    println!("   Database: {}", app_config.database_type);
    println!("   OS: {}", platform_utils::get_os_name());
    println!("   Log Level: {}\n", app_config.log_level);

    let state = CommandState::new_with_token(auth_token);

    // 尝试从后台管理系统同步 DLP 规则
    println!("🔄 Syncing DLP rules from admin backend...");
    {
        let admin_url = std::env::var("ADMIN_BACKEND_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:3000".to_string());
        
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(3))
            .build()
            .ok();
        
        if let Some(client) = client {
            match client.get(format!("{}/api/dlp-rules", admin_url)).send().await {
                Ok(response) if response.status().is_success() => {
                    if let Ok(rules) = response.json::<serde_json::Value>().await {
                        // 支持 {"rules": [...]} 和 [...] 两种格式
                        let rules_array = if let Some(arr) = rules.as_array() {
                            arr.clone()
                        } else if let Some(arr) = rules.get("rules").and_then(|v| v.as_array()) {
                            arr.clone()
                        } else {
                            Vec::new()
                        };
                        
                        if !rules_array.is_empty() {
                            let mut custom_patterns = Vec::new();
                            for rule in &rules_array {
                                let enabled = rule["enabled"].as_bool().unwrap_or(true);
                                if !enabled { continue; }
                                
                                let name = rule["name"].as_str().unwrap_or("").to_string();
                                let raw_pattern = rule["pattern"].as_str().unwrap_or("").to_string();
                                let severity = rule["severity"].as_str().unwrap_or("Medium").to_string();
                                let description = rule["description"].as_str().map(|s| s.to_string());
                                
                                if raw_pattern.is_empty() { continue; }
                                
                                // 处理 /pattern/ 格式
                                let pattern = if raw_pattern.starts_with('/') && raw_pattern.ends_with('/') && raw_pattern.len() > 2 {
                                    raw_pattern[1..raw_pattern.len() - 1].to_string()
                                } else {
                                    raw_pattern
                                };
                                
                                // 验证正则表达式
                                if regex::Regex::new(&pattern).is_err() {
                                    eprintln!("⚠️  Skipping invalid DLP rule '{}': bad regex", name);
                                    continue;
                                }
                                
                                let action = match severity.as_str() {
                                    "critical" | "Critical" => "Block",
                                    "high" | "High" => "Redact",
                                    _ => "Redact",
                                };
                                
                                custom_patterns.push(
                                    desktop_client::dlp::DlpIntegrationConfig::custom_pattern(
                                        name, pattern, severity, action.to_string(), description,
                                    )
                                );
                            }
                            
                            if !custom_patterns.is_empty() {
                                let dlp = state.dlp_integration.lock().await;
                                let mut config = dlp.get_config().await;
                                config.custom_patterns = custom_patterns.clone();
                                if let Err(e) = dlp.update_config(config).await {
                                    eprintln!("⚠️  Failed to apply DLP rules: {}", e);
                                } else {
                                    println!("✅ Synced {} DLP rules from admin backend", custom_patterns.len());
                                }
                            } else {
                                println!("ℹ️  No enabled DLP rules found in admin backend");
                            }
                        } else {
                            println!("ℹ️  No DLP rules found in admin backend");
                        }
                    }
                }
                Ok(response) => {
                    eprintln!("⚠️  Admin backend returned {}, DLP rules not synced", response.status());
                }
                Err(e) => {
                    eprintln!("⚠️  Admin backend unreachable ({}), using default DLP rules", e);
                }
            }
        }
    }

    // 初始化 SSE 订阅管理器
    let sse_manager = Arc::new(Mutex::new(desktop_client::commands::SseSubscriptionManager::new()));

    tauri::Builder::default()
        .manage(state)
        .manage(sse_manager)
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
            delete_memory_local,
            is_memory_file_protected,
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
            // DLP 管理命令
            scan_user_input,
            scan_outbound_request,
            sanitize_for_storage,
            check_http_request,
            get_dlp_config,
            update_dlp_config,
            get_dlp_statistics,
            sync_dlp_rules_from_admin,
            // 聊天命令 (Tauri IPC)
            send_chat_message,
            subscribe_chat_events,
            unsubscribe_chat_events,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
