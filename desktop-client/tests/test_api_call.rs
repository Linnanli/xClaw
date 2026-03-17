/// 测试实际的 API 调用
/// 
/// 这个测试验证使用清理后的 token 能否成功调用后端 API

#[tokio::test]
async fn test_get_threads_with_real_token() {
    use std::path::PathBuf;
    
    // 读取数据库中的 token
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
        println!("Database not found, skipping test");
        return;
    }
    
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
    
    // 清理 token
    let cleaned_token = token
        .trim()
        .trim_matches('"')
        .trim()
        .to_string();
    
    println!("Using token: {}...{}", &cleaned_token[..8], &cleaned_token[cleaned_token.len()-8..]);
    
    // 创建 HTTP 客户端
    let client = reqwest::Client::new();
    let url = "http://localhost:3000/api/chat/threads";
    
    println!("Calling API: {}", url);
    
    // 发送请求
    let response = match client
        .get(url)
        .header("Authorization", format!("Bearer {}", cleaned_token))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            println!("❌ Request failed: {}", e);
            println!("   Make sure the backend is running: cargo run -- run --no-onboard");
            return;
        }
    };
    
    let status = response.status();
    println!("Response status: {}", status);
    
    if !status.is_success() {
        let error_text = response.text().await.unwrap_or_default();
        println!("❌ API call failed: {}", error_text);
        panic!("API call failed with status {}: {}", status, error_text);
    }
    
    let body = response.text().await.unwrap();
    println!("✅ API call successful!");
    println!("Response (first 200 chars): {}", &body[..body.len().min(200)]);
    
    // 验证响应是有效的 JSON
    match serde_json::from_str::<serde_json::Value>(&body) {
        Ok(_) => println!("✅ Response is valid JSON"),
        Err(e) => panic!("❌ Response is not valid JSON: {}", e),
    }
}
