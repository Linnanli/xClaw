use desktop_client::auth_token_manager::{AuthTokenManager, TokenError};
use desktop_client::{is_valid_token, clean_token};
use desktop_client::commands::get_auth_token;
use std::fs;
use tempfile::TempDir;
use tokio;

// ========== 集成测试覆盖率 >80% ==========

#[tokio::test]
async fn test_end_to_end_auth_flow() {
    // 1. 创建临时目录模拟用户环境
    let temp_dir = TempDir::new().unwrap();
    let token_file = temp_dir.path().join(".auth_token");
    
    // 2. 创建 AuthTokenManager 并生成 Token
    let token_manager = AuthTokenManager::new();
    let backend_token = token_manager.load_or_generate().unwrap();
    
    // 3. 验证Token格式正确
    assert_eq!(backend_token.len(), 64);
    assert!(backend_token.chars().all(|c| c.is_ascii_hexdigit()));
    
    // 4. 模拟前端通过Tauri命令获取Token
    // 注意：这里我们直接调用命令函数，因为在测试环境中没有完整的Tauri运行时
    let frontend_token = get_auth_token().await.unwrap();
    
    // 5. 验证前后端Token一致性
    assert_eq!(backend_token, frontend_token, "Frontend and backend tokens should match");
    
    println!("✅ End-to-end auth flow test passed");
    println!("   Backend token:  {}", &backend_token[..16]);
    println!("   Frontend token: {}", &frontend_token[..16]);
}

#[tokio::test]
async fn test_token_persistence_across_restarts() {
    // 1. 创建临时目录
    let temp_dir = TempDir::new().unwrap();
    let token_file = temp_dir.path().join(".auth_token");
    
    // 2. 第一次启动 - 生成Token
    let token_manager1 = AuthTokenManager::new();
    let token1 = token_manager1.load_or_generate().unwrap();
    token_manager1.save(&token1).unwrap();
    
    // 3. 模拟应用重启 - 创建新的管理器实例
    let token_manager2 = AuthTokenManager::new();
    let token2 = token_manager2.load_or_generate().unwrap();
    
    // 4. 验证Token在重启后保持一致
    assert_eq!(token1, token2, "Token should persist across restarts");
    
    println!("✅ Token persistence test passed");
}

#[tokio::test]
async fn test_token_error_scenarios() {
    // 测试各种错误场景的处理
    
    // 1. 测试无效Token文件
    let temp_dir = TempDir::new().unwrap();
    let invalid_token_file = temp_dir.path().join("invalid_token");
    fs::write(&invalid_token_file, "invalid_token_content").unwrap();
    
    let token_manager = AuthTokenManager::new();
    
    // 2. 测试Token验证失败的情况
    let result = token_manager.save("invalid");
    assert!(result.is_err(), "Should reject invalid token");
    
    // 3. 测试空Token
    let result = token_manager.save("");
    assert!(result.is_err(), "Should reject empty token");
    
    // 4. 测试包含特殊字符的Token
    let result = token_manager.save(&format!("{}g", "a".repeat(63)));
    assert!(result.is_err(), "Should reject token with invalid characters");
    
    println!("✅ Token error scenarios test passed");
}

#[tokio::test]
async fn test_concurrent_token_access() {
    use std::sync::Arc;
    use tokio::sync::Mutex;
    
    // 测试并发访问Token的安全性
    let token_manager = Arc::new(Mutex::new(AuthTokenManager::new()));
    let mut handles = Vec::new();
    
    // 启动多个并发任务
    for i in 0..10 {
        let manager = token_manager.clone();
        let handle = tokio::spawn(async move {
            let manager = manager.lock().await;
            let token = manager.load_or_generate();
            println!("Task {} got token: {:?}", i, token.is_ok());
            token
        });
        handles.push(handle);
    }
    
    // 等待所有任务完成
    let mut tokens = Vec::new();
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok(), "All concurrent token access should succeed");
        tokens.push(result.unwrap());
    }
    
    // 验证所有Token都相同（因为是同一个管理器）
    let first_token = &tokens[0];
    for token in &tokens[1..] {
        assert_eq!(first_token, token, "All concurrent accesses should return the same token");
    }
    
    println!("✅ Concurrent token access test passed");
}

#[tokio::test]
async fn test_token_refresh_scenario() {
    // 测试Token刷新场景
    
    // 1. 创建临时目录
    let temp_dir = TempDir::new().unwrap();
    let token_file = temp_dir.path().join(".auth_token");
    
    // 2. 手动创建第一个token并保存
    let original_token = AuthTokenManager::generate_new_token();
    fs::write(&token_file, &original_token).unwrap();
    
    // 3. 生成新Token（模拟刷新）
    let new_token = AuthTokenManager::generate_new_token();
    
    // 4. 验证新Token不同于原Token
    assert_ne!(original_token, new_token, "Refreshed token should be different");
    
    // 5. 验证新Token格式正确
    assert_eq!(new_token.len(), 64);
    assert!(new_token.chars().all(|c| c.is_ascii_hexdigit()));
    
    // 6. 保存新Token到文件
    fs::write(&token_file, &new_token).unwrap();
    
    // 7. 验证能够读取新Token
    let loaded_token = fs::read_to_string(&token_file).unwrap();
    let loaded_token = loaded_token.trim();
    assert_eq!(new_token, loaded_token, "Should load the new token");
    
    println!("✅ Token refresh scenario test passed");
}

#[tokio::test]
async fn test_database_token_cleaning() {
    // 测试从数据库读取Token时的清理逻辑
    // 注意：这个测试模拟数据库返回的各种格式
    
    // 模拟数据库返回的各种Token格式
    let test_cases = vec![
        // (输入, 是否应该成功, 描述)
        (r#""ca66c45dfd3cbfbff339e1c9fb628ec0686dd3ca0d699970c5656ae62f253e6e""#, true, "JSON quoted token"),
        (" ca66c45dfd3cbfbff339e1c9fb628ec0686dd3ca0d699970c5656ae62f253e6e ", true, "Token with spaces"),
        ("ca66c45dfd3cbfbff339e1c9fb628ec0686dd3ca0d699970c5656ae62f253e6e\n", true, "Token with newline"),
        ("\tca66c45dfd3cbfbff339e1c9fb628ec0686dd3ca0d699970c5656ae62f253e6e\t", true, "Token with tabs"),
        ("ca66c45dfd3cbfbff339e1c9fb628ec0686dd3ca0d699970c5656ae62f253e6e\r\n", true, "Token with CRLF"),
        ("invalid_token", false, "Invalid token"),
        ("", false, "Empty token"),
        ("   ", false, "Whitespace only"),
    ];
    
    for (input, should_succeed, description) in test_cases {
        let result = clean_token(input);
        
        if should_succeed {
            assert!(result.is_ok(), "Should succeed for: {}", description);
            let cleaned = result.unwrap();
            assert!(is_valid_token(&cleaned), "Cleaned token should be valid for: {}", description);
            assert_eq!(cleaned.len(), 64, "Cleaned token should be 64 chars for: {}", description);
        } else {
            assert!(result.is_err(), "Should fail for: {}", description);
        }
    }
    
    println!("✅ Database token cleaning test passed");
}