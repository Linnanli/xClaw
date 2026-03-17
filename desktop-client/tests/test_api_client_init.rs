/// 测试 ApiClient 初始化和 API 调用
/// 
/// 验证 ApiClient 能否正确读取 token 并成功调用后端 API

#[tokio::test]
async fn test_api_client_initialization() {
    use desktop_client::api_client::ApiClient;
    
    // 初始化 tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .try_init();
    
    println!("\n=== Testing ApiClient Initialization ===\n");
    
    // 创建 ApiClient（会自动读取 token）
    let client = ApiClient::new("http://localhost:3000".to_string());
    
    println!("\n=== Testing API Call ===\n");
    
    // 调用 get_threads API
    match client.get_threads().await {
        Ok(response) => {
            println!("✅ API call successful!");
            println!("   Assistant thread: {:?}", response.assistant_thread.map(|t| t.id));
            println!("   Threads count: {}", response.threads.len());
        }
        Err(e) => {
            println!("❌ API call failed: {:?}", e);
            println!("\n请确保后端正在运行：cargo run -- run --no-onboard");
            panic!("API call failed: {:?}", e);
        }
    }
}

#[test]
fn test_api_client_token_loading() {
    use desktop_client::AuthTokenManager;
    
    println!("\n=== Testing Token Loading ===\n");
    
    let token_manager = AuthTokenManager::new();
    
    match token_manager.load_or_generate() {
        Ok(token) => {
            println!("✅ Token loaded successfully");
            println!("   Length: {}", token.len());
            println!("   First 16 chars: {}", &token[..token.len().min(16)]);
            println!("   Last 16 chars: {}", &token[token.len().saturating_sub(16)..]);
            
            // 验证 token 格式
            assert_eq!(token.len(), 64, "Token should be 64 characters");
            assert!(token.chars().all(|c| c.is_ascii_hexdigit()), "Token should only contain hex digits");
            
            // 验证 HTTP header 格式
            let header = format!("Bearer {}", token);
            match reqwest::header::HeaderValue::from_str(&header) {
                Ok(_) => println!("✅ HTTP Header is valid"),
                Err(e) => panic!("❌ HTTP Header is invalid: {}", e),
            }
        }
        Err(e) => {
            println!("❌ Failed to load token: {}", e);
            panic!("Token loading failed: {}", e);
        }
    }
}
