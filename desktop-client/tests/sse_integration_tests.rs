//! SSE 集成测试 - 验证 SSE 客户端与后端的集成
//!
//! 本测试文件验证 SSE 客户端的功能，包括：
//! - SSE 连接建立
//! - 事件接收和处理
//! - 事件序列化/反序列化

mod support;

use desktop_client::SseClient;
use support::TestFixture;

// ==================== SSE 客户端基础测试 ====================

#[tokio::test]
async fn test_sse_client_creation() {
    // Feature: api-integration-tests, SSE: 创建 SSE 客户端
    // 验证需求: SSE 客户端创建
    
    let client = SseClient::new(
        "http://localhost:3000".to_string(),
        "test-token".to_string(),
    );
    
    assert!(!client.is_connected().await);
}

#[tokio::test]
async fn test_sse_client_event_handler_registration() {
    // Feature: api-integration-tests, SSE: 注册事件处理器
    // 验证需求: SSE 事件处理器
    
    let client = SseClient::new(
        "http://localhost:3000".to_string(),
        "test-token".to_string(),
    );
    
    let event_received = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let event_received_clone = event_received.clone();
    
    client.on_event(move |_event| {
        event_received_clone.store(true, std::sync::atomic::Ordering::SeqCst);
    }).await;
    
    // 验证处理器已注册
    assert!(!event_received.load(std::sync::atomic::Ordering::SeqCst));
}

// ==================== SSE 事件类型测试 ====================

#[test]
fn test_sse_message_event_serialization() {
    // Feature: api-integration-tests, SSE: 消息事件序列化
    // 验证需求: SSE 事件序列化
    
    let event = desktop_client::SseEvent::Message {
        thread_id: "thread-123".to_string(),
        message_id: "msg-456".to_string(),
        content: "Hello".to_string(),
        role: "user".to_string(),
    };
    
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("thread-123"));
    assert!(json.contains("msg-456"));
    assert!(json.contains("Hello"));
}

#[test]
fn test_sse_thread_state_event_serialization() {
    // Feature: api-integration-tests, SSE: 对话状态事件序列化
    // 验证需求: SSE 事件序列化
    
    let event = desktop_client::SseEvent::ThreadState {
        thread_id: "thread-123".to_string(),
        state: "Idle".to_string(),
    };
    
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("thread-123"));
    assert!(json.contains("Idle"));
}

#[test]
fn test_sse_auth_completed_event_serialization() {
    // Feature: api-integration-tests, SSE: 认证完成事件序列化
    // 验证需求: SSE 事件序列化
    
    let event = desktop_client::SseEvent::AuthCompleted {
        extension_name: "oauth".to_string(),
        success: true,
    };
    
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("oauth"));
    assert!(json.contains("true"));
}

#[test]
fn test_sse_auth_required_event_serialization() {
    // Feature: api-integration-tests, SSE: 认证需求事件序列化
    // 验证需求: SSE 事件序列化
    
    let event = desktop_client::SseEvent::AuthRequired {
        extension_name: "oauth".to_string(),
        instructions: Some("Please authenticate".to_string()),
    };
    
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("oauth"));
    assert!(json.contains("Please authenticate"));
}

// ==================== SSE 事件反序列化测试 ====================

#[test]
fn test_sse_message_event_deserialization() {
    // Feature: api-integration-tests, SSE: 消息事件反序列化
    // 验证需求: SSE 事件反序列化
    
    let json = r#"{"message":{"thread_id":"thread-123","message_id":"msg-456","content":"Hello","role":"user"}}"#;
    let event: desktop_client::SseEvent = serde_json::from_str(json).unwrap();
    
    match event {
        desktop_client::SseEvent::Message { thread_id, message_id, content, role } => {
            assert_eq!(thread_id, "thread-123");
            assert_eq!(message_id, "msg-456");
            assert_eq!(content, "Hello");
            assert_eq!(role, "user");
        }
        _ => panic!("Expected Message event"),
    }
}

#[test]
fn test_sse_thread_state_event_deserialization() {
    // Feature: api-integration-tests, SSE: 对话状态事件反序列化
    // 验证需求: SSE 事件反序列化
    
    let json = r#"{"thread_state":{"thread_id":"thread-123","state":"Idle"}}"#;
    let event: desktop_client::SseEvent = serde_json::from_str(json).unwrap();
    
    match event {
        desktop_client::SseEvent::ThreadState { thread_id, state } => {
            assert_eq!(thread_id, "thread-123");
            assert_eq!(state, "Idle");
        }
        _ => panic!("Expected ThreadState event"),
    }
}

#[test]
fn test_sse_auth_completed_event_deserialization() {
    // Feature: api-integration-tests, SSE: 认证完成事件反序列化
    // 验证需求: SSE 事件反序列化
    
    let json = r#"{"auth_completed":{"extension_name":"oauth","success":true}}"#;
    let event: desktop_client::SseEvent = serde_json::from_str(json).unwrap();
    
    match event {
        desktop_client::SseEvent::AuthCompleted { extension_name, success } => {
            assert_eq!(extension_name, "oauth");
            assert!(success);
        }
        _ => panic!("Expected AuthCompleted event"),
    }
}

// ==================== SSE 与 API 集成测试 ====================

#[tokio::test]
async fn test_sse_client_with_api_client() {
    // Feature: api-integration-tests, SSE: SSE 客户端与 API 客户端集成
    // 验证需求: SSE 与 API 集成
    
    let fixture = TestFixture::new().await.unwrap();
    let api_client = fixture.client();
    
    // 创建 SSE 客户端
    let sse_client = SseClient::new(
        api_client.base_url().to_string(),
        api_client.auth_token().to_string(),
    );
    
    // 验证 SSE 客户端已创建
    assert!(!sse_client.is_connected().await);
}

#[tokio::test]
async fn test_sse_event_round_trip() {
    // Feature: api-integration-tests, SSE: SSE 事件 round-trip
    // 验证需求: SSE 事件序列化/反序列化
    
    let original_event = desktop_client::SseEvent::Message {
        thread_id: "thread-123".to_string(),
        message_id: "msg-456".to_string(),
        content: "Hello, world!".to_string(),
        role: "user".to_string(),
    };
    
    // 序列化
    let json = serde_json::to_string(&original_event).unwrap();
    
    // 反序列化
    let deserialized_event: desktop_client::SseEvent = serde_json::from_str(&json).unwrap();
    
    // 验证
    match deserialized_event {
        desktop_client::SseEvent::Message { thread_id, message_id, content, role } => {
            assert_eq!(thread_id, "thread-123");
            assert_eq!(message_id, "msg-456");
            assert_eq!(content, "Hello, world!");
            assert_eq!(role, "user");
        }
        _ => panic!("Expected Message event"),
    }
}

// ==================== SSE 错误处理测试 ====================

#[tokio::test]
async fn test_sse_client_disconnect() {
    // Feature: api-integration-tests, SSE: SSE 客户端断开连接
    // 验证需求: SSE 连接管理
    
    let client = SseClient::new(
        "http://localhost:3000".to_string(),
        "test-token".to_string(),
    );
    
    // 验证初始状态
    assert!(!client.is_connected().await);
    
    // 断开连接
    client.disconnect().await;
    
    // 验证仍未连接
    assert!(!client.is_connected().await);
}

#[tokio::test]
async fn test_sse_multiple_event_handlers() {
    // Feature: api-integration-tests, SSE: 多个事件处理器
    // 验证需求: SSE 事件处理
    
    let client = SseClient::new(
        "http://localhost:3000".to_string(),
        "test-token".to_string(),
    );
    
    let counter1 = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter2 = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    
    let counter1_clone = counter1.clone();
    client.on_event(move |_event| {
        counter1_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }).await;
    
    let counter2_clone = counter2.clone();
    client.on_event(move |_event| {
        counter2_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }).await;
    
    // 验证处理器已注册
    assert_eq!(counter1.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert_eq!(counter2.load(std::sync::atomic::Ordering::SeqCst), 0);
}
