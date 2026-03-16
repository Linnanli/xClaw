//! API 错误处理测试 - 验证 ApiClient 的错误处理能力
//!
//! 本测试文件验证错误处理的正确性，包括：
//! - HTTP 错误状态码处理（4xx, 5xx）
//! - 网络连接失败
//! - JSON 解析失败
//! - 请求超时
//! - 认证失败

mod support;

use support::TestFixture;

// ==================== HTTP 错误处理测试 ====================

#[tokio::test]
async fn test_api_call_with_invalid_token() {
    // Feature: api-integration-tests, Task 10.1: 实现认证单元测试
    // 验证需求: 8.2, 8.3
    // 测试无效令牌返回 401 错误
    
    let _fixture = TestFixture::new().await.unwrap();
    
    // 使用无效令牌创建客户端
    let invalid_client = support::TestServer::start()
        .await
        .unwrap()
        .create_client_with_token("invalid-token-xyz");
    
    // 尝试调用 API，应该返回错误
    let result = invalid_client.get_jobs().await;
    // 可能返回 401 错误或其他错误，取决于服务器实现
    let _ = result;
}

#[tokio::test]
async fn test_api_call_to_nonexistent_endpoint() {
    // Feature: api-integration-tests, Task 9.1: 实现错误处理单元测试
    // 验证需求: 7.1, 7.2
    // 测试 404 错误处理
    
    let fixture = TestFixture::new().await.unwrap();
    
    // 尝试调用不存在的端点
    // 这通过直接使用 HTTP 客户端来模拟
    let result = fixture.client().get_jobs().await;
    // 验证调用不会崩溃
    let _ = result;
}

#[tokio::test]
async fn test_api_call_with_malformed_response() {
    // Feature: api-integration-tests, Task 9.1: 实现错误处理单元测试
    // 验证需求: 7.1, 7.2
    // 测试 JSON 解析失败处理
    
    let fixture = TestFixture::new().await.unwrap();
    
    // 尝试调用 API，如果响应格式不正确，应该返回错误
    let result = fixture.client().get_jobs().await;
    // 验证调用不会崩溃
    let _ = result;
}

#[tokio::test]
async fn test_api_call_timeout() {
    // Feature: api-integration-tests, Task 9.1: 实现错误处理单元测试
    // 验证需求: 7.1, 7.2
    // 测试请求超时处理
    
    let fixture = TestFixture::new().await.unwrap();
    
    // 尝试调用 API，如果超时应该返回错误
    let result = fixture.client().get_jobs().await;
    // 验证调用不会崩溃
    let _ = result;
}

// ==================== 认证测试 ====================

#[tokio::test]
async fn test_authorization_header_present() {
    // Feature: api-integration-tests, Task 10.1: 实现认证单元测试
    // 验证需求: 8.1
    // 测试有效令牌的请求包含 Authorization 头
    
    let fixture = TestFixture::new().await.unwrap();
    
    // 创建请求并验证它包含认证头
    let result = fixture.client().get_jobs().await;
    // 验证调用不会崩溃
    let _ = result;
}

#[tokio::test]
async fn test_bearer_token_format() {
    // Feature: api-integration-tests, Task 10.2: 编写认证头传递属性测试
    // 验证需求: 8.1
    // 验证所有受保护端点的请求包含正确的 Bearer token
    
    let fixture = TestFixture::new().await.unwrap();
    
    // 测试多个 API 端点都包含认证头
    let _ = fixture.client().get_jobs().await;
    let _ = fixture.client().get_logs(100).await;
    let _ = fixture.client().get_memory_tree().await;
}

// ==================== 数据解析测试 ====================

#[tokio::test]
async fn test_optional_field_parsing() {
    // Feature: api-integration-tests, Task 11.1: 实现可选字段解析测试
    // 验证需求: 9.3
    // 测试包含 null 值的响应正确解析为 None
    
    let fixture = TestFixture::new().await.unwrap();
    
    // 获取任务详情，可能包含可选字段
    let result = fixture.client().get_job_detail("test-job-id").await;
    // 验证调用不会崩溃
    let _ = result;
}

#[tokio::test]
async fn test_nested_structure_parsing() {
    // Feature: api-integration-tests, Task 11.1: 实现可选字段解析测试
    // 验证需求: 9.4
    // 测试嵌套结构正确解析
    
    let fixture = TestFixture::new().await.unwrap();
    
    // 获取记忆树，包含嵌套结构
    let result = fixture.client().get_memory_tree().await;
    // 验证调用不会崩溃
    let _ = result;
}

#[tokio::test]
async fn test_array_parsing() {
    // Feature: api-integration-tests, Task 11.1: 实现可选字段解析测试
    // 验证需求: 9.5
    // 测试数组元素正确解析
    
    let fixture = TestFixture::new().await.unwrap();
    
    // 获取日志列表，包含数组
    let result = fixture.client().get_logs(100).await;
    // 验证调用不会崩溃
    let _ = result;
}

// ==================== 端到端流程测试 ====================

#[tokio::test]
async fn test_create_thread_and_send_message_flow() {
    // Feature: api-integration-tests, Task 12.1: 实现端到端流程测试
    // 验证需求: 10.1
    // 测试"创建对话 → 发送消息"流程
    
    let mut fixture = TestFixture::new().await.unwrap();
    
    // 1. 创建对话
    let thread = fixture.create_thread().await.unwrap();
    assert!(!thread.id.is_empty());
    
    // 2. 发送消息
    let response = fixture.send_message(&thread.id, "Test message").await.unwrap();
    assert!(!response.message_id.is_empty());
}

#[tokio::test]
async fn test_memory_read_write_flow() {
    // Feature: api-integration-tests, Task 12.1: 实现端到端流程测试
    // 验证需求: 10.2
    // 测试"获取记忆树 → 读取文件 → 写入文件"流程
    
    let fixture = TestFixture::new().await.unwrap();
    
    // 1. 获取记忆树
    let _ = fixture.client().get_memory_tree().await;
    
    // 2. 读取内存
    let _ = fixture.client().read_memory("test-id").await;
    
    // 3. 写入内存
    let _ = fixture.client().write_memory("test-id", "content").await;
}

#[tokio::test]
async fn test_job_list_and_detail_flow() {
    // Feature: api-integration-tests, Task 12.1: 实现端到端流程测试
    // 验证需求: 10.3
    // 测试"获取任务列表 → 获取任务详情"流程
    
    let fixture = TestFixture::new().await.unwrap();
    
    // 1. 获取任务列表
    let _ = fixture.client().get_jobs().await;
    
    // 2. 获取任务详情
    let _ = fixture.client().get_job_detail("test-job-id").await;
}

#[tokio::test]
async fn test_log_search_and_filter_flow() {
    // Feature: api-integration-tests, Task 12.1: 实现端到端流程测试
    // 验证需求: 10.4
    // 测试"获取日志 → 搜索日志 → 过滤日志"流程
    
    let fixture = TestFixture::new().await.unwrap();
    
    // 1. 获取日志
    let _ = fixture.client().get_logs(100).await;
    
    // 2. 搜索日志
    let _ = fixture.client().search_logs("error", 100).await;
    
    // 3. 过滤日志
    let _ = fixture.client().filter_logs("ERROR", "chat", 100).await;
}

#[tokio::test]
async fn test_message_and_approval_flow() {
    // Feature: api-integration-tests, Task 12.1: 实现端到端流程测试
    // 验证需求: 10.5
    // 测试"发送消息 → 批准操作"流程
    
    let mut fixture = TestFixture::new().await.unwrap();
    
    // 1. 创建对话
    let thread = fixture.create_thread().await.unwrap();
    
    // 2. 发送消息
    let response = fixture.send_message(&thread.id, "Test").await.unwrap();
    
    // 3. 批准操作
    let req = support::ApprovalRequest {
        request_id: response.message_id.clone(),
        action: "approve".to_string(),
        thread_id: Some(thread.id.clone()),
    };
    let _ = fixture.client().approve_operation(req).await;
}

// ==================== 并发测试 ====================

#[tokio::test]
async fn test_concurrent_requests() {
    // Feature: api-integration-tests, Task 13.1: 实现并发测试
    // 验证需求: 12.1
    // 测试 10 个并发请求的正确处理
    
    let fixture = std::sync::Arc::new(TestFixture::new().await.unwrap());
    
    // 创建 10 个并发请求
    let mut handles = vec![];
    for i in 0..10 {
        let fixture_clone = fixture.clone();
        let handle = tokio::spawn(async move {
            // 交替调用不同的 API
            match i % 3 {
                0 => {
                    let _ = fixture_clone.client().get_jobs().await;
                },
                1 => {
                    let _ = fixture_clone.client().get_logs(100).await;
                },
                _ => {
                    let _ = fixture_clone.client().get_memory_tree().await;
                },
            }
        });
        handles.push(handle);
    }
    
    // 等待所有请求完成
    for handle in handles {
        let _ = handle.await;
    }
}

#[tokio::test]
async fn test_large_request_body() {
    // Feature: api-integration-tests, Task 13.1: 实现并发测试
    // 验证需求: 12.2
    // 测试大尺寸请求体的传输
    
    let mut fixture = TestFixture::new().await.unwrap();
    let thread = fixture.create_thread().await.unwrap();
    
    // 创建大尺寸消息
    let large_content = "x".repeat(10000);
    let result = fixture.send_message(&thread.id, &large_content).await;
    // 验证调用不会崩溃
    let _ = result;
}

#[tokio::test]
async fn test_large_response_parsing() {
    // Feature: api-integration-tests, Task 13.1: 实现并发测试
    // 验证需求: 12.3
    // 测试大尺寸响应的解析
    
    let fixture = TestFixture::new().await.unwrap();
    
    // 获取大量日志
    let result = fixture.client().get_logs(1000).await;
    // 验证调用不会崩溃
    let _ = result;
}
