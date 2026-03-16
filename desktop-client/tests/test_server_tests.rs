//! TestServer 组件的单元测试

mod support;

use support::TestServer;

#[tokio::test]
async fn test_server_starts_successfully() {
    let server = TestServer::start().await;
    assert!(server.is_ok(), "Server should start successfully");
}

#[tokio::test]
async fn test_server_binds_to_dynamic_port() {
    let server = TestServer::start().await.unwrap();
    let addr = server.addr();
    
    assert_eq!(addr.ip().to_string(), "127.0.0.1");
    assert_ne!(addr.port(), 0, "Port should be assigned");
}

#[tokio::test]
async fn test_base_url_format() {
    let server = TestServer::start().await.unwrap();
    let base_url = server.base_url();
    
    assert!(base_url.starts_with("http://127.0.0.1:"));
    assert!(base_url.contains(&server.addr().port().to_string()));
}

#[tokio::test]
async fn test_create_client() {
    let server = TestServer::start().await.unwrap();
    let client = server.create_client();
    
    // ApiClient 应该配置了正确的 base_url
    // 这里我们只能验证 client 被成功创建
    drop(client);
}

#[tokio::test]
async fn test_custom_auth_token() {
    let custom_token = "my-custom-token-456";
    let server = TestServer::start_with_token(custom_token).await.unwrap();
    
    assert_eq!(server.auth_token(), custom_token);
}

#[tokio::test]
async fn test_multiple_servers_different_ports() {
    let server1 = TestServer::start().await.unwrap();
    let server2 = TestServer::start().await.unwrap();
    
    assert_ne!(
        server1.addr().port(),
        server2.addr().port(),
        "Different servers should use different ports"
    );
}

#[tokio::test]
async fn test_manual_shutdown() {
    let server = TestServer::start().await.unwrap();
    let result = server.shutdown().await;
    
    assert!(result.is_ok(), "Manual shutdown should succeed");
}

#[tokio::test]
async fn test_server_auto_cleanup_on_drop() {
    let addr = {
        let server = TestServer::start().await.unwrap();
        server.addr()
    }; // server 在这里被 drop
    
    // 等待一小段时间让服务器完成关闭
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    
    // 验证端口已经释放（尝试绑定到相同端口应该成功）
    let listener = tokio::net::TcpListener::bind(addr).await;
    assert!(listener.is_ok(), "Port should be released after drop");
}
