/// 测试 token 清理逻辑
/// 
/// 这个测试验证从数据库读取的 JSON 字符串格式的 token 能否正确清理

#[test]
fn test_token_cleaning_from_db_format() {
    // 模拟从数据库读取的 JSON 字符串格式（包含引号）
    let db_value = r#""ca66c45dfd3cbfbff339e1c9fb628ec0686dd3ca0d699970c5656ae62f253e6e""#;
    
    // 清理逻辑（与 auth_token_manager.rs 中的逻辑一致）
    let cleaned = db_value
        .trim()
        .trim_matches('"')
        .trim()
        .to_string();
    
    println!("Original: {:?} (len={})", db_value, db_value.len());
    println!("Cleaned:  {:?} (len={})", cleaned, cleaned.len());
    
    // 验证
    assert_eq!(cleaned.len(), 64, "Token should be 64 characters after cleaning");
    assert!(cleaned.chars().all(|c| c.is_ascii_hexdigit()), "Token should only contain hex digits");
    assert!(!cleaned.contains('"'), "Token should not contain quotes");
    assert!(!cleaned.contains('\n'), "Token should not contain newlines");
    assert!(!cleaned.contains('\r'), "Token should not contain carriage returns");
    assert!(!cleaned.contains(' '), "Token should not contain spaces");
    
    // 验证清理后的 token 是预期的值
    assert_eq!(cleaned, "ca66c45dfd3cbfbff339e1c9fb628ec0686dd3ca0d699970c5656ae62f253e6e");
}

#[test]
fn test_token_with_extra_whitespace() {
    // 测试包含额外空白字符的情况
    let db_value = r#"  "ca66c45dfd3cbfbff339e1c9fb628ec0686dd3ca0d699970c5656ae62f253e6e"  "#;
    
    let cleaned = db_value
        .trim()
        .trim_matches('"')
        .trim()
        .to_string();
    
    println!("With whitespace - Original: {:?}", db_value);
    println!("With whitespace - Cleaned:  {:?}", cleaned);
    
    assert_eq!(cleaned.len(), 64);
    assert_eq!(cleaned, "ca66c45dfd3cbfbff339e1c9fb628ec0686dd3ca0d699970c5656ae62f253e6e");
}

#[test]
fn test_http_header_format() {
    // 测试 HTTP Authorization header 格式
    let token = "ca66c45dfd3cbfbff339e1c9fb628ec0686dd3ca0d699970c5656ae62f253e6e";
    let header = format!("Bearer {}", token);
    
    println!("Header: {:?} (len={})", header, header.len());
    
    // 验证 header 格式
    assert_eq!(header, "Bearer ca66c45dfd3cbfbff339e1c9fb628ec0686dd3ca0d699970c5656ae62f253e6e");
    assert_eq!(header.len(), 71); // "Bearer " (7) + token (64)
    
    // 验证 header 不包含非法字符
    assert!(!header.contains('\n'), "Header should not contain newlines");
    assert!(!header.contains('\r'), "Header should not contain carriage returns");
    assert!(!header.contains('"'), "Header should not contain quotes");
}

#[test]
fn test_actual_db_token() {
    // 使用实际的数据库 token 进行测试
    use std::path::PathBuf;
    
    let home_dir = match std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
        Ok(dir) => dir,
        Err(_) => {
            println!("Cannot determine home directory, skipping test");
            return;
        }
    };
    
    let db_path = PathBuf::from(home_dir)
        .join(".ironclaw")
        .join("ironclaw.db");
    
    if !db_path.exists() {
        println!("Database not found at {:?}, skipping test", db_path);
        return;
    }
    
    // 读取数据库中的 token
    let conn = match rusqlite::Connection::open(&db_path) {
        Ok(c) => c,
        Err(e) => {
            println!("Failed to open database: {}, skipping test", e);
            return;
        }
    };
    
    let token: String = match conn.query_row(
        "SELECT value FROM settings WHERE key = 'channels.gateway_auth_token' LIMIT 1",
        [],
        |row| row.get(0),
    ) {
        Ok(t) => t,
        Err(e) => {
            println!("Failed to query token: {}, skipping test", e);
            return;
        }
    };
    
    println!("Raw token from DB: {:?} (len={})", token, token.len());
    
    // 清理 token
    let cleaned = token
        .trim()
        .trim_matches('"')
        .trim()
        .to_string();
    
    println!("Cleaned token: {:?} (len={})", cleaned, cleaned.len());
    
    // 验证
    assert_eq!(cleaned.len(), 64, "Token should be 64 characters");
    assert!(cleaned.chars().all(|c| c.is_ascii_hexdigit()), "Token should only contain hex digits");
    
    // 测试 HTTP header
    let header = format!("Bearer {}", cleaned);
    println!("HTTP Header: {:?} (len={})", header, header.len());
    
    // 验证 header 可以被 reqwest 接受
    match reqwest::header::HeaderValue::from_str(&header) {
        Ok(_) => println!("✅ Header is valid"),
        Err(e) => panic!("❌ Header is invalid: {}", e),
    }
}
