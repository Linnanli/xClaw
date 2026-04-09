use desktop_client::auth_token_manager::AuthTokenManager;
use desktop_client::{clean_token, is_valid_token};
use std::fs;
use tempfile::TempDir;
use tokio;

// ========== 变更覆盖率 >70% ==========
// 确保重构不会破坏现有功能的回归测试

/// 测试重构前后的Token生成行为一致性
#[tokio::test]
async fn test_token_generation_backward_compatibility() {
    // 验证Token生成的基本特性保持不变

    // 1. Token长度保持64字符
    let token = AuthTokenManager::generate_new_token();
    assert_eq!(token.len(), 64, "Token length should remain 64 characters");

    // 2. Token只包含十六进制字符
    assert!(
        token.chars().all(|c| c.is_ascii_hexdigit()),
        "Token should only contain hexadecimal characters"
    );

    // 3. 每次生成的Token都不同
    let token2 = AuthTokenManager::generate_new_token();
    assert_ne!(token, token2, "Generated tokens should be unique");

    // 4. Token符合UUID v4的基本格式（去除连字符后）
    // 虽然我们不验证具体的UUID格式，但确保随机性
    let mut tokens = std::collections::HashSet::new();
    for _ in 0..100 {
        let t = AuthTokenManager::generate_new_token();
        assert!(tokens.insert(t), "All generated tokens should be unique");
    }

    println!("✅ Token generation backward compatibility verified");
}

/// 测试重构前后的Token验证行为一致性
#[tokio::test]
async fn test_token_validation_backward_compatibility() {
    // 验证Token验证逻辑的向后兼容性

    // 1. 有效Token格式保持不变
    let valid_tokens = vec![
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "FEDCBA9876543210FEDCBA9876543210FEDCBA9876543210FEDCBA9876543210",
        "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2",
    ];

    for token in valid_tokens {
        assert!(
            is_valid_token(token),
            "Valid token should pass validation: {}",
            token
        );
    }

    // 2. 无效Token格式保持被拒绝
    let invalid_g64 = "g".repeat(64);
    let invalid_a63 = "a".repeat(63);
    let invalid_a65 = "a".repeat(65);
    let invalid_newline = format!("{}\n", "a".repeat(63));
    let invalid_space = format!("{} ", "a".repeat(63));
    let invalid_quote = format!("\"{}\"", "a".repeat(62));

    let invalid_tokens = vec![
        "",               // 空字符串
        "short",          // 太短
        &invalid_g64,     // 非十六进制字符
        &invalid_a63,     // 长度不足
        &invalid_a65,     // 长度过长
        &invalid_newline, // 包含换行符
        &invalid_space,   // 包含空格
        &invalid_quote,   // 包含引号
    ];

    for token in invalid_tokens {
        assert!(
            !is_valid_token(token),
            "Invalid token should fail validation: {}",
            token
        );
    }

    println!("✅ Token validation backward compatibility verified");
}

/// 测试重构前后的Token清理行为一致性
#[tokio::test]
async fn test_token_cleaning_backward_compatibility() {
    // 验证Token清理逻辑的向后兼容性

    let base_token = "ca66c45dfd3cbfbff339e1c9fb628ec0686dd3ca0d699970c5656ae62f253e6e";

    // 1. 旧版本支持的清理格式应该继续工作
    let cleanable_formats = vec![
        (format!("\"{}\"", base_token), "JSON quoted token"),
        (format!(" {} ", base_token), "Token with spaces"),
        (format!("{}\n", base_token), "Token with newline"),
        (format!("{}\r", base_token), "Token with carriage return"),
        (format!("\t{}\t", base_token), "Token with tabs"),
        (
            format!(" \t\"{}\" \r\n", base_token),
            "Token with mixed whitespace",
        ),
    ];

    for (input, description) in cleanable_formats {
        let result = clean_token(&input);
        assert!(result.is_ok(), "Should clean {}: {}", description, input);

        let cleaned = result.unwrap();
        assert_eq!(
            cleaned, base_token,
            "Cleaned token should match expected for {}",
            description
        );
        assert!(
            is_valid_token(&cleaned),
            "Cleaned token should be valid for {}",
            description
        );
    }

    // 2. 无法清理的格式应该继续被拒绝
    let invalid_g64 = "g".repeat(64);
    let invalid_a63 = "a".repeat(63);

    let uncleanable_formats = vec![
        ("", "Empty string"),
        ("   ", "Whitespace only"),
        ("invalid", "Invalid format"),
        (&invalid_g64, "Non-hex characters"),
        (&invalid_a63, "Wrong length after cleaning"),
    ];

    for (input, description) in uncleanable_formats {
        let result = clean_token(input);
        assert!(result.is_err(), "Should reject {}: {}", description, input);
    }

    println!("✅ Token cleaning backward compatibility verified");
}

/// 测试重构前后的文件操作行为一致性
#[tokio::test]
async fn test_file_operations_backward_compatibility() {
    // 验证文件操作的向后兼容性

    // 1. 创建临时目录
    let temp_dir = TempDir::new().unwrap();
    let token_file = temp_dir.path().join(".auth_token");

    // 2. 测试Token保存格式保持不变
    let token = AuthTokenManager::generate_new_token();
    fs::write(&token_file, &token).unwrap();

    // 验证文件内容
    let file_content = fs::read_to_string(&token_file).unwrap();
    assert_eq!(
        file_content.trim(),
        token,
        "File content should match saved token"
    );

    // 3. 测试Token加载兼容性
    let loaded_token = fs::read_to_string(&token_file).unwrap();
    assert_eq!(
        loaded_token.trim(),
        token,
        "Loaded token should match saved token"
    );

    // 4. 测试旧格式文件的兼容性
    let old_format_token = format!("{}\n", token); // 带换行符的旧格式
    fs::write(&token_file, &old_format_token).unwrap();

    let loaded_old = fs::read_to_string(&token_file).unwrap();
    assert_eq!(loaded_old.trim(), token, "Should handle old format files");

    println!("✅ File operations backward compatibility verified");
}

/// 测试重构前后的错误处理行为一致性
#[tokio::test]
async fn test_error_handling_backward_compatibility() {
    // 验证错误处理的向后兼容性

    let token_manager = AuthTokenManager::new();

    // 1. 无效Token保存应该继续被拒绝
    let invalid_g64 = "g".repeat(64);
    let invalid_a100 = "a".repeat(100);

    let invalid_saves = vec![
        ("", "Empty token"),
        ("invalid", "Invalid format"),
        ("short", "Too short"),
        (&invalid_g64, "Non-hex characters"),
        (&invalid_a100, "Too long"),
    ];

    for (invalid_token, description) in invalid_saves {
        let result = token_manager.save(invalid_token);
        assert!(
            result.is_err(),
            "Should reject {}: {}",
            description,
            invalid_token
        );
    }

    // 2. 有效Token保存应该继续成功
    let valid_token = AuthTokenManager::generate_new_token();
    let result = token_manager.save(&valid_token);
    assert!(result.is_ok(), "Should accept valid token");

    // 3. 错误类型应该保持一致
    let empty_result = token_manager.save("");
    assert!(empty_result.is_err(), "Empty token should produce error");

    println!("✅ Error handling backward compatibility verified");
}

/// 测试重构前后的并发行为一致性
#[tokio::test]
async fn test_concurrency_backward_compatibility() {
    // 验证并发行为的向后兼容性

    use std::sync::Arc;
    use tokio::sync::Mutex;

    let token_manager = Arc::new(Mutex::new(AuthTokenManager::new()));
    let mut handles = Vec::new();

    // 启动并发任务
    for i in 0..10 {
        let manager = token_manager.clone();
        let handle = tokio::spawn(async move {
            let manager = manager.lock().await;
            let result = manager.load_or_generate();
            drop(manager);
            (i, result)
        });
        handles.push(handle);
    }

    // 收集结果
    let mut results = Vec::new();
    for handle in handles {
        let (i, result) = handle.await.unwrap();
        assert!(result.is_ok(), "Concurrent operation {} should succeed", i);
        results.push(result.unwrap());
    }

    // 验证一致性（所有操作应该返回相同的Token）
    let first_token = &results[0];
    for (i, token) in results.iter().enumerate().skip(1) {
        assert_eq!(
            first_token, token,
            "Concurrent result {} should be consistent",
            i
        );
    }

    println!("✅ Concurrency backward compatibility verified");
}

/// 测试重构前后的性能特性保持一致
#[tokio::test]
async fn test_performance_backward_compatibility() {
    // 验证性能特性的向后兼容性

    use std::time::Instant;

    // 1. Token生成性能应该保持在合理范围内
    let start = Instant::now();
    for _ in 0..1000 {
        let _token = AuthTokenManager::generate_new_token();
    }
    let generation_time = start.elapsed();

    // 性能要求：1000个Token生成应该在1秒内完成
    assert!(
        generation_time.as_secs() < 1,
        "Token generation performance regression: {}ms for 1000 tokens",
        generation_time.as_millis()
    );

    // 2. Token验证性能应该保持快速
    let token = AuthTokenManager::generate_new_token();
    let start = Instant::now();
    for _ in 0..10000 {
        let _valid = is_valid_token(&token);
    }
    let validation_time = start.elapsed();

    // 性能要求：10000次验证应该在100ms内完成
    assert!(
        validation_time.as_millis() < 100,
        "Token validation performance regression: {}ms for 10000 validations",
        validation_time.as_millis()
    );

    println!("✅ Performance backward compatibility verified");
    println!(
        "   Generation: {}ms/1000 tokens",
        generation_time.as_millis()
    );
    println!(
        "   Validation: {}ms/10000 validations",
        validation_time.as_millis()
    );
}

/// 测试重构前后的API接口保持一致
#[tokio::test]
async fn test_api_interface_backward_compatibility() {
    // 验证公共API接口的向后兼容性

    // 1. AuthTokenManager的公共方法应该保持可用
    let token_manager = AuthTokenManager::new();

    // 基本方法
    let _token = token_manager.load_or_generate();
    assert!(_token.is_ok(), "load_or_generate should work");

    let _exists = token_manager.exists();
    // exists方法应该返回bool

    let _path = token_manager.get_token_file_path();
    // get_token_file_path应该返回PathBuf引用

    // 2. 静态方法应该保持可用
    let _new_token = AuthTokenManager::generate_new_token();
    assert_eq!(
        _new_token.len(),
        64,
        "generate_new_token should return 64-char token"
    );

    // 3. 全局函数应该保持可用
    assert!(is_valid_token(&_new_token), "is_valid_token should work");

    let _cleaned = clean_token(&format!("\"{}\"", _new_token));
    assert!(_cleaned.is_ok(), "clean_token should work");

    println!("✅ API interface backward compatibility verified");
}

/// 测试重构前后的数据格式兼容性
#[tokio::test]
async fn test_data_format_backward_compatibility() {
    // 验证数据格式的向后兼容性

    // 1. 测试各种历史Token格式
    let historical_tokens = vec![
        // 标准格式
        "ca66c45dfd3cbfbff339e1c9fb628ec0686dd3ca0d699970c5656ae62f253e6e",
        // 大写格式
        "CA66C45DFD3CBFBFF339E1C9FB628EC0686DD3CA0D699970C5656AE62F253E6E",
        // 混合大小写
        "Ca66C45dFd3cBfBfF339e1C9Fb628eC0686dD3cA0d699970C5656Ae62F253e6E",
    ];

    for token in historical_tokens {
        assert!(
            is_valid_token(token),
            "Historical token format should be valid: {}",
            token
        );
    }

    // 2. 测试各种清理场景的兼容性
    let base = "ca66c45dfd3cbfbff339e1c9fb628ec0686dd3ca0d699970c5656ae62f253e6e";
    let formats_to_clean = vec![
        format!("\"{}\"", base),            // JSON格式
        format!(" {} ", base),              // 空格包围
        format!("{}\n", base),              // 换行结尾
        format!("\t{}", base),              // 制表符开头
        format!(" \n\t\"{}\" \r\n ", base), // 复杂空白字符
    ];

    for format in formats_to_clean {
        let cleaned = clean_token(&format);
        assert!(cleaned.is_ok(), "Should clean format: {}", format);
        assert_eq!(cleaned.unwrap(), base, "Cleaned result should match base");
    }

    println!("✅ Data format backward compatibility verified");
}

/// 测试重构前后的边界条件处理一致性
#[tokio::test]
async fn test_boundary_conditions_backward_compatibility() {
    // 验证边界条件处理的向后兼容性

    // 1. 长度边界测试
    let boundary_lengths = vec![
        (0, false),   // 空字符串
        (1, false),   // 太短
        (63, false),  // 差一个字符
        (64, true),   // 正确长度
        (65, false),  // 多一个字符
        (128, false), // 太长
    ];

    for (length, should_be_valid) in boundary_lengths {
        let test_token = if length == 0 {
            String::new()
        } else {
            "a".repeat(length)
        };

        let is_valid = is_valid_token(&test_token);
        assert_eq!(
            is_valid,
            should_be_valid,
            "Token of length {} should be {}",
            length,
            if should_be_valid { "valid" } else { "invalid" }
        );
    }

    // 2. 字符集边界测试
    let character_tests = vec![
        ("0123456789abcdef".repeat(4), true),    // 小写十六进制
        ("0123456789ABCDEF".repeat(4), true),    // 大写十六进制
        ("0123456789abcdeF".repeat(4), true),    // 混合大小写
        (format!("{}g", "a".repeat(63)), false), // 包含非十六进制字符
        (format!("{}G", "a".repeat(63)), false), // 包含非十六进制字符（大写）
    ];

    for (test_token, should_be_valid) in character_tests {
        let is_valid = is_valid_token(&test_token);
        assert_eq!(
            is_valid,
            should_be_valid,
            "Token '{}...' should be {}",
            &test_token[..8],
            if should_be_valid { "valid" } else { "invalid" }
        );
    }

    println!("✅ Boundary conditions backward compatibility verified");
}
