//! TestFixture 单元测试
//!
//! 验证 TestFixture 组件的功能：
//! - 测试数据创建
//! - 资源清理机制

mod support;

use support::TestFixture;

#[tokio::test]
async fn test_fixture_creates_successfully() {
    // Feature: api-integration-tests, Task 1.3: 实现 TestFixture 组件
    // 验证 TestFixture 可以成功创建
    
    let fixture = TestFixture::new().await;
    assert!(fixture.is_ok(), "TestFixture should create successfully: {:?}", fixture.err());
    
    let _fixture = fixture.unwrap();
    // 只验证 fixture 创建成功，不测试 API 调用（因为需要完整的数据库初始化）
}

#[tokio::test]
async fn test_fixture_creates_thread() {
    // Feature: api-integration-tests, Task 1.3: 实现 TestFixture 组件
    // 验证 TestFixture 可以创建测试对话
    
    let mut fixture = TestFixture::new().await.unwrap();
    
    let thread = fixture.create_thread().await;
    assert!(thread.is_ok(), "Should create thread successfully");
    
    let thread = thread.unwrap();
    assert!(!thread.id.is_empty(), "Thread should have non-empty ID");
}

#[tokio::test]
async fn test_fixture_sends_message() {
    // Feature: api-integration-tests, Task 1.3: 实现 TestFixture 组件
    // 验证 TestFixture 可以发送测试消息
    
    let mut fixture = TestFixture::new().await.unwrap();
    let thread = fixture.create_thread().await.unwrap();
    
    let response = fixture.send_message(&thread.id, "Test message").await;
    assert!(response.is_ok(), "Should send message successfully: {:?}", response.err());
    
    let response = response.unwrap();
    assert!(!response.message_id.is_empty(), "Response should have message ID");
}

#[tokio::test]
async fn test_fixture_cleanup() {
    // Feature: api-integration-tests, Task 1.3: 实现 TestFixture 组件
    // 验证 TestFixture 的清理机制
    
    let mut fixture = TestFixture::new().await.unwrap();
    
    // 创建一些资源
    let _thread1 = fixture.create_thread().await.unwrap();
    let _thread2 = fixture.create_thread().await.unwrap();
    
    // 手动清理
    let result = fixture.cleanup().await;
    assert!(result.is_ok(), "Cleanup should succeed");
}

#[tokio::test]
async fn test_fixture_with_custom_token() {
    // Feature: api-integration-tests, Task 1.3: 实现 TestFixture 组件
    // 验证 TestFixture 可以使用自定义认证令牌
    
    let custom_token = "custom-test-token-456";
    let fixture = TestFixture::new_with_token(custom_token).await;
    assert!(fixture.is_ok(), "TestFixture should create with custom token");
    
    let fixture = fixture.unwrap();
    assert_eq!(fixture.server().auth_token(), custom_token, "Should use custom token");
}
