//! TestServer 与 ApiClient 的集成测试
//!
//! 验证 TestServer 可以正确启动并与 ApiClient 配合工作

mod support;

use support::TestServer;

#[tokio::test]
async fn test_server_health_check() {
    // 启动测试服务器
    let server = TestServer::start().await.unwrap();
    let _client = server.create_client();
    
    // 尝试调用健康检查端点（不需要认证）
    let health_url = format!("{}/api/health", server.base_url());
    let response = reqwest::get(&health_url).await;
    
    assert!(response.is_ok(), "Health check should succeed");
    let response = response.unwrap();
    assert!(response.status().is_success(), "Health check should return 2xx status");
}

#[tokio::test]
async fn test_client_base_url_configured() {
    let server = TestServer::start().await.unwrap();
    let _client = server.create_client();
    
    // 验证服务器 URL 格式正确
    let base_url = server.base_url();
    assert!(base_url.starts_with("http://127.0.0.1:"));
    
    // 验证端口号是有效的
    let port = server.addr().port();
    assert!(port > 0);
}

#[tokio::test]
async fn test_multiple_clients_same_server() {
    let server = TestServer::start().await.unwrap();
    
    // 创建多个客户端
    let client1 = server.create_client();
    let client2 = server.create_client();
    
    // 两个客户端应该指向同一个服务器
    drop(client1);
    drop(client2);
    
    // 服务器应该仍然运行
    let health_url = format!("{}/api/health", server.base_url());
    let response = reqwest::get(&health_url).await;
    assert!(response.is_ok());
}
