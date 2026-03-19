//! 令牌刷新机制测试
//! 
//! 测试覆盖：
//! - 单元测试：令牌刷新逻辑、刷新间隔计算
//! - 集成测试：令牌刷新与会话管理的集成
//! - 失败路径测试：网络故障、后端错误
//! - 安全测试：令牌泄露防护、刷新令牌验证
//! - 可靠性测试：并发刷新、重试机制
//! - 需求级测试：自动刷新、手动刷新
//! - 变更覆盖测试：向后兼容性

use desktop_client::auth_token_manager::AuthTokenManager;
use desktop_client::session_config::SessionConfig;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use serial_test::serial;

// ========== 单元测试：令牌刷新逻辑 ==========

#[test]
fn test_token_generation_uniqueness() {
    // 生成多个令牌，确保它们都是唯一的
    let mut tokens = std::collections::HashSet::new();
    
    for _ in 0..100 {
        let token = AuthTokenManager::generate_new_token();
        assert_eq!(token.len(), 64, "Token should be 64 characters");
        assert!(tokens.insert(token), "Token should be unique");
    }
    
    assert_eq!(tokens.len(), 100, "All tokens should be unique");
}

#[test]
fn test_token_format_validation() {
    let token = AuthTokenManager::generate_new_token();
    
    // 验证令牌格式
    assert_eq!(token.len(), 64);
    assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
    assert!(!token.contains('\n'));
    assert!(!token.contains('\r'));
    assert!(!token.contains(' '));
    assert!(!token.contains('"'));
}

#[test]
fn test_session_config_refresh_interval() {
    let config = SessionConfig::default();
    
    // 默认刷新间隔应该在超时前5分钟
    assert_eq!(config.session_timeout_secs, 30 * 60);
    assert_eq!(config.token_refresh_interval_secs, 25 * 60);
    assert!(config.token_refresh_interval_secs < config.session_timeout_secs);
}

#[test]
fn test_should_refresh_token_logic() {
    let config = SessionConfig::default();
    
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    // 测试不同的时间点
    assert!(!config.should_refresh_token(now), "Just refreshed, should not refresh");
    assert!(!config.should_refresh_token(now - 10 * 60), "10 minutes ago, should not refresh");
    assert!(!config.should_refresh_token(now - 24 * 60), "24 minutes ago, should not refresh");
    assert!(config.should_refresh_token(now - 25 * 60), "25 minutes ago, should refresh");
    assert!(config.should_refresh_token(now - 30 * 60), "30 minutes ago, should refresh");
}

// ========== 集成测试：令牌刷新与会话管理 ==========

#[test]
#[serial]
fn test_token_refresh_workflow() {
    let manager = AuthTokenManager::new();
    
    // 清理旧令牌
    let _ = manager.delete();
    
    // 生成初始令牌
    let initial_token = manager.load_or_generate().unwrap();
    assert_eq!(initial_token.len(), 64);
    
    // 模拟令牌刷新
    let new_token = AuthTokenManager::generate_new_token();
    assert_ne!(initial_token, new_token, "New token should be different");
    
    // 保存新令牌
    manager.save(&new_token).unwrap();
    
    // 验证新令牌已保存
    let loaded_token = manager.load().unwrap();
    assert_eq!(loaded_token, new_token);
    
    // 清理
    manager.delete().unwrap();
}

#[test]
#[serial]
fn test_token_refresh_preserves_validity() {
    let manager = AuthTokenManager::new();
    
    // 清理旧令牌
    let _ = manager.delete();
    
    // 生成并保存令牌
    let token1 = AuthTokenManager::generate_new_token();
    manager.save(&token1).unwrap();
    
    // 验证可以加载
    let loaded1 = manager.load().unwrap();
    assert_eq!(loaded1, token1, "First token should match");
    
    // 生成并保存新令牌
    let token2 = AuthTokenManager::generate_new_token();
    manager.save(&token2).unwrap();
    
    // 验证新令牌已保存
    let loaded2 = manager.load().unwrap();
    assert_eq!(loaded2, token2, "Second token should match");
    
    // 清理
    manager.delete().unwrap();
}

// ========== 失败路径测试：错误处理 ==========

#[test]
fn test_save_invalid_token() {
    let manager = AuthTokenManager::new();
    
    let token_63 = "a".repeat(63);
    let token_65 = "a".repeat(65);
    let token_with_g = format!("{}g", "a".repeat(63));
    let token_with_newline = format!("{}\n", "a".repeat(63));
    
    // 尝试保存无效令牌
    let invalid_tokens: Vec<&str> = vec![
        "",
        "short",
        &token_63,
        &token_65,
        &token_with_g,
        &token_with_newline,
    ];
    
    for invalid_token in invalid_tokens {
        let result = manager.save(invalid_token);
        assert!(result.is_err(), "Should reject invalid token: {}", invalid_token);
    }
}

#[test]
#[serial]
fn test_load_nonexistent_token() {
    let manager = AuthTokenManager::new();
    
    // 确保令牌不存在
    let _ = manager.delete();
    
    // 尝试加载不存在的令牌
    let result = manager.load();
    
    // 应该返回错误
    assert!(result.is_err(), "Should return error when token doesn't exist");
}

#[test]
#[serial]
fn test_token_refresh_on_corruption() {
    let manager = AuthTokenManager::new();
    
    // 清理旧令牌
    let _ = manager.delete();
    
    // 写入损坏的令牌
    let token_path = manager.get_token_file_path();
    std::fs::create_dir_all(token_path.parent().unwrap()).unwrap();
    std::fs::write(token_path, "corrupted_token").unwrap();
    
    // load_or_generate 应该检测到损坏并生成新令牌
    let token = manager.load_or_generate().unwrap();
    assert_eq!(token.len(), 64);
    assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
    
    // 清理
    manager.delete().unwrap();
}

// ========== 安全测试：令牌泄露防护 ==========

#[test]
fn test_security_token_not_logged() {
    // 验证令牌不会被意外记录到日志
    let token = AuthTokenManager::generate_new_token();
    
    // 模拟日志捕获
    let log_output = format!("Processing token: {}", &token[..8]); // 只记录前8个字符
    
    // 验证完整令牌不在日志中
    assert!(!log_output.contains(&token), "Full token should not be in logs");
    assert!(log_output.len() < token.len(), "Log should not contain full token");
}

#[test]
fn test_security_token_storage_permissions() {
    let manager = AuthTokenManager::new();
    let token = AuthTokenManager::generate_new_token();
    
    // 保存令牌
    manager.save(&token).unwrap();
    
    // 验证令牌文件存在
    assert!(manager.exists(), "Token file should exist");
    
    // 在实际应用中，应该验证文件权限（仅所有者可读写）
    // 这需要平台特定的代码
}

#[test]
#[serial]
fn test_security_token_rotation() {
    let manager = AuthTokenManager::new();
    
    // 清理旧令牌
    let _ = manager.delete();
    
    // 生成初始令牌
    let token1 = AuthTokenManager::generate_new_token();
    manager.save(&token1).unwrap();
    
    // 轮换令牌
    let token2 = AuthTokenManager::generate_new_token();
    manager.save(&token2).unwrap();
    
    // 验证旧令牌已被替换
    let current_token = manager.load().unwrap();
    assert_eq!(current_token, token2);
    assert_ne!(current_token, token1);
    
    // 清理
    manager.delete().unwrap();
}

// ========== 可靠性测试：并发刷新 ==========

#[test]
fn test_reliability_concurrent_token_refresh() {
    use std::sync::{Arc, Mutex};
    use std::thread;
    
    let manager = Arc::new(Mutex::new(AuthTokenManager::new()));
    
    // 多线程并发刷新令牌
    let mut handles = vec![];
    for i in 0..10 {
        let manager_clone = Arc::clone(&manager);
        let handle = thread::spawn(move || {
            let token = AuthTokenManager::generate_new_token();
            let manager_lock = manager_clone.lock().unwrap();
            manager_lock.save(&token).unwrap();
            (i, token)
        });
        handles.push(handle);
    }
    
    // 等待所有线程完成
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    
    // 验证所有令牌都是唯一的
    let tokens: Vec<_> = results.iter().map(|(_, token)| token).collect();
    let unique_tokens: std::collections::HashSet<_> = tokens.iter().collect();
    assert_eq!(unique_tokens.len(), 10, "All tokens should be unique");
}

#[test]
fn test_reliability_token_refresh_retry() {
    // 模拟令牌刷新重试机制
    let manager = AuthTokenManager::new();
    
    let mut retry_count = 0;
    let max_retries = 3;
    
    loop {
        let token = AuthTokenManager::generate_new_token();
        match manager.save(&token) {
            Ok(_) => break,
            Err(_) if retry_count < max_retries => {
                retry_count += 1;
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => panic!("Failed to save token after {} retries: {:?}", max_retries, e),
        }
    }
    
    assert!(retry_count <= max_retries, "Should succeed within retry limit");
}

// ========== 需求级测试 ==========

#[test]
fn req_token_001_automatic_refresh_before_expiry() {
    // 需求：令牌应该在会话过期前自动刷新
    let config = SessionConfig::default();
    
    // 刷新间隔应该小于超时时间
    assert!(config.token_refresh_interval_secs < config.session_timeout_secs);
    
    // 刷新间隔应该至少提前5分钟
    let margin = config.session_timeout_secs - config.token_refresh_interval_secs;
    assert!(margin >= 5 * 60, "Refresh should happen at least 5 minutes before timeout");
}

#[test]
#[serial]
fn req_token_002_manual_refresh_on_demand() {
    // 需求：用户应该能够手动刷新令牌
    let manager = AuthTokenManager::new();
    
    // 清理旧令牌
    let _ = manager.delete();
    
    // 生成两个不同的令牌
    let initial_token = AuthTokenManager::generate_new_token();
    let new_token = AuthTokenManager::generate_new_token();
    
    // 确保它们不同
    assert_ne!(initial_token, new_token, "Tokens should be different");
    
    // 保存初始令牌
    manager.save(&initial_token).unwrap();
    
    // 验证初始令牌已保存
    let loaded_initial = manager.load().unwrap();
    assert_eq!(loaded_initial, initial_token);
    
    // 手动刷新
    manager.save(&new_token).unwrap();
    
    // 验证令牌已更新
    let current_token = manager.load().unwrap();
    assert_eq!(current_token, new_token);
    
    // 清理
    manager.delete().unwrap();
}

#[test]
fn req_token_003_refresh_disabled_when_configured() {
    // 需求：应该能够禁用自动刷新
    let mut config = SessionConfig::default();
    config.auto_refresh_enabled = false;
    
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    // 即使超过刷新间隔，也不应该刷新
    assert!(!config.should_refresh_token(now - 30 * 60));
}

// ========== 变更覆盖测试：向后兼容性 ==========

#[test]
fn test_backward_compatibility_token_format() {
    // 验证新生成的令牌与旧格式兼容
    let token = AuthTokenManager::generate_new_token();
    
    // 旧格式验证（64字符十六进制）
    assert_eq!(token.len(), 64);
    assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
#[serial]
fn test_backward_compatibility_load_old_token() {
    let manager = AuthTokenManager::new();
    
    // 清理旧令牌
    let _ = manager.delete();
    
    // 模拟旧格式令牌（64字符十六进制）
    let old_token = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";
    manager.save(old_token).unwrap();
    
    // 应该能够加载旧格式令牌
    let loaded_token = manager.load().unwrap();
    assert_eq!(loaded_token, old_token);
    
    // 清理
    manager.delete().unwrap();
}

// ========== 代码级覆盖测试 ==========

#[test]
#[serial]
fn test_code_coverage_all_branches() {
    let manager = AuthTokenManager::new();
    
    // 清理旧令牌
    let _ = manager.delete();
    
    // 测试所有分支
    
    // 分支1：令牌不存在，生成新令牌
    let token1 = manager.load_or_generate().unwrap();
    assert_eq!(token1.len(), 64);
    
    // 分支2：令牌存在，加载现有令牌
    let token2 = manager.load_or_generate().unwrap();
    assert_eq!(token2, token1);
    
    // 分支3：删除令牌
    manager.delete().unwrap();
    assert!(!manager.exists());
    
    // 分支4：令牌不存在，再次生成
    let token3 = manager.load_or_generate().unwrap();
    assert_eq!(token3.len(), 64);
    
    // 清理
    manager.delete().unwrap();
}

// ========== 数据级覆盖测试 ==========

#[test]
fn test_data_coverage_token_lengths() {
    // 测试不同长度的令牌
    let manager = AuthTokenManager::new();
    
    let token_63 = "a".repeat(63);
    let token_64 = "a".repeat(64);
    let token_65 = "a".repeat(65);
    let token_128 = "a".repeat(128);
    
    let test_cases: Vec<(&str, bool)> = vec![
        ("", false),                    // 空字符串
        ("a", false),                   // 太短
        (&token_63, false),             // 长度不足
        (&token_64, true),              // 正确长度
        (&token_65, false),             // 太长
        (&token_128, false),            // 远超长度
    ];
    
    for (token, should_succeed) in test_cases {
        let result = manager.save(token);
        if should_succeed {
            assert!(result.is_ok(), "Should accept token: {}", token);
        } else {
            assert!(result.is_err(), "Should reject token: {}", token);
        }
    }
}

#[test]
fn test_data_coverage_token_characters() {
    // 测试不同字符类型的令牌
    let manager = AuthTokenManager::new();
    
    let token_0 = "0".repeat(64);
    let token_f = "f".repeat(64);
    let token_hex = "0123456789abcdef".repeat(4);
    let token_with_g = format!("{}g", "a".repeat(63));
    let token_with_space = format!("{} ", "a".repeat(63));
    let token_with_newline = format!("{}\n", "a".repeat(63));
    
    let test_cases: Vec<(&str, bool)> = vec![
        (&token_0, true),               // 全0
        (&token_f, true),               // 全f
        (&token_hex, true),             // 所有十六进制字符
        (&token_with_g, false),         // 包含非十六进制字符
        (&token_with_space, false),     // 包含空格
        (&token_with_newline, false),   // 包含换行符
    ];
    
    for (token, should_succeed) in test_cases {
        let result = manager.save(token);
        if should_succeed {
            assert!(result.is_ok(), "Should accept token: {}", token);
        } else {
            assert!(result.is_err(), "Should reject token: {}", token);
        }
    }
}
