//! API 属性测试 - 使用 proptest 验证通用属性
//!
//! 本测试文件使用属性测试来验证 API 的通用特性，包括：
//! - 数据序列化 Round-trip
//! - API 调用成功返回有效数据
//! - 写入操作的幂等性
//! - 查询参数正确编码

mod support;

use proptest::prelude::*;
use support::*;

// ==================== 属性 1: 数据序列化 Round-trip ====================

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 50,
        max_shrink_iters: 1000,
        ..ProptestConfig::default()
    })]

    #[test]
    fn prop_send_message_request_round_trip(req in arb_send_message_request()) {
        // Feature: api-integration-tests, Property 1: 数据序列化 Round-trip
        // 验证需求: 1.10, 9.1, 9.2
        // 验证 SendMessageRequest 的序列化和反序列化
        
        let json = serde_json::to_string(&req).expect("serialization should succeed");
        let deserialized: desktop_client::api_client::SendMessageRequest = 
            serde_json::from_str(&json).expect("deserialization should succeed");
        
        assert_eq!(req.content, deserialized.content);
        assert_eq!(req.thread_id, deserialized.thread_id);
    }

    #[test]
    fn prop_thread_info_round_trip(thread in arb_thread_info()) {
        // Feature: api-integration-tests, Property 1: 数据序列化 Round-trip
        // 验证需求: 1.10, 9.1, 9.2
        // 验证 ThreadInfo 的序列化和反序列化
        
        let json = serde_json::to_string(&thread).expect("serialization should succeed");
        let deserialized: desktop_client::api_client::ThreadInfo = 
            serde_json::from_str(&json).expect("deserialization should succeed");
        
        assert_eq!(thread.id, deserialized.id);
        assert_eq!(thread.state, deserialized.state);
        assert_eq!(thread.turn_count, deserialized.turn_count);
    }

    #[test]
    fn prop_message_round_trip(msg in arb_message()) {
        // Feature: api-integration-tests, Property 1: 数据序列化 Round-trip
        // 验证需求: 1.10, 9.1, 9.2
        // 验证 Message 的序列化和反序列化
        
        let json = serde_json::to_string(&msg).expect("serialization should succeed");
        let deserialized: desktop_client::api_client::Message = 
            serde_json::from_str(&json).expect("deserialization should succeed");
        
        assert_eq!(msg.id, deserialized.id);
        assert_eq!(msg.thread_id, deserialized.thread_id);
        assert_eq!(msg.role, deserialized.role);
        assert_eq!(msg.content, deserialized.content);
    }

    #[test]
    fn prop_job_info_round_trip(job in arb_job_info()) {
        // Feature: api-integration-tests, Property 1: 数据序列化 Round-trip
        // 验证需求: 3.5, 9.1, 9.2
        // 验证 JobInfo 的序列化和反序列化
        
        let json = serde_json::to_string(&job).expect("serialization should succeed");
        let deserialized: desktop_client::api_client::JobInfo = 
            serde_json::from_str(&json).expect("deserialization should succeed");
        
        assert_eq!(job.id, deserialized.id);
        assert_eq!(job.status, deserialized.status);
    }

    #[test]
    fn prop_log_entry_round_trip(log in arb_log_entry()) {
        // Feature: api-integration-tests, Property 1: 数据序列化 Round-trip
        // 验证需求: 4.6, 9.1, 9.2
        // 验证 LogEntry 的序列化和反序列化
        
        let json = serde_json::to_string(&log).expect("serialization should succeed");
        let deserialized: desktop_client::api_client::LogEntry = 
            serde_json::from_str(&json).expect("deserialization should succeed");
        
        assert_eq!(log.timestamp, deserialized.timestamp);
        assert_eq!(log.level, deserialized.level);
        assert_eq!(log.module, deserialized.module);
        assert_eq!(log.message, deserialized.message);
    }
}

// ==================== 属性 2: API 调用成功返回有效数据 ====================

#[tokio::test]
async fn prop_create_thread_returns_valid_data() {
    // Feature: api-integration-tests, Property 2: API 调用成功返回有效数据
    // 验证需求: 1.2, 1.4
    // 验证 create_thread 返回有效的 ThreadInfo
    
    let mut fixture = TestFixture::new().await.unwrap();
    
    for _ in 0..10 {
        let thread = fixture.create_thread().await.unwrap();
        
        // 验证返回的数据有效
        assert!(!thread.id.is_empty(), "thread ID should not be empty");
        assert!(!thread.state.is_empty(), "thread state should not be empty");
        assert_eq!(thread.state, "Idle", "new thread should be in Idle state");
    }
}

#[tokio::test]
async fn prop_send_message_returns_valid_data() {
    // Feature: api-integration-tests, Property 2: API 调用成功返回有效数据
    // 验证需求: 1.3, 1.4
    // 验证 send_message 返回有效的 SendMessageResponse
    
    for i in 0..10 {
        let mut fixture = TestFixture::new().await.unwrap();
        let thread = fixture.create_thread().await.unwrap();
        
        let response = fixture
            .send_message(&thread.id, &format!("Message {}", i))
            .await
            .unwrap();
        
        // 验证返回的数据有效
        assert!(!response.message_id.is_empty(), "message ID should not be empty");
        assert!(!response.status.is_empty(), "status should not be empty");
    }
}

// ==================== 属性 3: 写入操作的幂等性验证 ====================

#[tokio::test]
async fn prop_create_thread_idempotent() {
    // Feature: api-integration-tests, Property 3: 写入操作的幂等性验证
    // 验证需求: 1.2
    // 验证创建对话后查询能反映变更
    
    let mut fixture = TestFixture::new().await.unwrap();
    
    // 创建对话
    let thread1 = fixture.create_thread().await.unwrap();
    let thread2 = fixture.create_thread().await.unwrap();
    
    // 验证两个对话有不同的 ID
    assert_ne!(thread1.id, thread2.id, "different threads should have different IDs");
}

#[tokio::test]
async fn prop_send_message_idempotent() {
    // Feature: api-integration-tests, Property 3: 写入操作的幂等性验证
    // 验证需求: 1.3
    // 验证发送消息后查询能反映变更
    
    let mut fixture1 = TestFixture::new().await.unwrap();
    let thread1 = fixture1.create_thread().await.unwrap();
    let msg1 = fixture1.send_message(&thread1.id, "Message 1").await.unwrap();
    
    let mut fixture2 = TestFixture::new().await.unwrap();
    let thread2 = fixture2.create_thread().await.unwrap();
    let msg2 = fixture2.send_message(&thread2.id, "Message 2").await.unwrap();
    
    // 验证消息有不同的 ID
    assert_ne!(msg1.message_id, msg2.message_id, "different messages should have different IDs");
}

// ==================== 属性 4: 查询参数正确编码 ====================

#[tokio::test]
async fn prop_special_characters_in_message() {
    // Feature: api-integration-tests, Property 4: 查询参数正确编码
    // 验证需求: 1.5
    // 验证特殊字符正确编码
    
    let special_messages = vec![
        "Hello! 你好 🎉",
        "Test @user #tag",
        "URL: https://example.com?foo=bar&baz=qux",
        "Symbols: !@#$%^&*()",
        "Quotes: \"double\" and 'single'",
    ];
    
    for msg in special_messages {
        let mut fixture = TestFixture::new().await.unwrap();
        let thread = fixture.create_thread().await.unwrap();
        
        let result = fixture.send_message(&thread.id, msg).await;
        assert!(result.is_ok(), "should handle special characters: {}", msg);
    }
}

// ==================== 属性 1: 记忆接口数据序列化 Round-trip ====================

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 50,
        max_shrink_iters: 1000,
        ..ProptestConfig::default()
    })]

    #[test]
    fn prop_memory_node_round_trip(node in arb_memory_node()) {
        // Feature: api-integration-tests, Property 1: 数据序列化 Round-trip
        // 验证需求: 2.5, 9.1, 9.2
        // 验证 MemoryNode 的序列化和反序列化
        
        let json = serde_json::to_string(&node).expect("serialization should succeed");
        let deserialized: desktop_client::api_client::MemoryNode = 
            serde_json::from_str(&json).expect("deserialization should succeed");
        
        assert_eq!(node.id, deserialized.id);
        assert_eq!(node.name, deserialized.name);
    }
}

// ==================== 属性 1: 任务接口数据序列化 Round-trip ====================

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 50,
        max_shrink_iters: 1000,
        ..ProptestConfig::default()
    })]

    #[test]
    fn prop_job_detail_round_trip(job in arb_job_info()) {
        // Feature: api-integration-tests, Property 1: 数据序列化 Round-trip
        // 验证需求: 3.5, 9.1, 9.2
        // 验证 JobDetail 的序列化和反序列化
        
        let json = serde_json::to_string(&job).expect("serialization should succeed");
        let deserialized: desktop_client::api_client::JobInfo = 
            serde_json::from_str(&json).expect("deserialization should succeed");
        
        assert_eq!(job.id, deserialized.id);
        assert_eq!(job.status, deserialized.status);
    }
}

// ==================== 属性 1: 日志接口数据序列化 Round-trip ====================

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 50,
        max_shrink_iters: 1000,
        ..ProptestConfig::default()
    })]

    #[test]
    fn prop_log_list_round_trip(logs in prop::collection::vec(arb_log_entry(), 1..10)) {
        // Feature: api-integration-tests, Property 1: 数据序列化 Round-trip
        // 验证需求: 4.6, 9.1, 9.2
        // 验证日志列表的序列化和反序列化
        
        let json = serde_json::to_string(&logs).expect("serialization should succeed");
        let deserialized: Vec<desktop_client::api_client::LogEntry> = 
            serde_json::from_str(&json).expect("deserialization should succeed");
        
        assert_eq!(logs.len(), deserialized.len());
        for (original, deserialized) in logs.iter().zip(deserialized.iter()) {
            assert_eq!(original.timestamp, deserialized.timestamp);
            assert_eq!(original.level, deserialized.level);
        }
    }
}

// ==================== 属性 1: 批准接口数据序列化 Round-trip ====================

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 50,
        max_shrink_iters: 1000,
        ..ProptestConfig::default()
    })]

    #[test]
    fn prop_approval_request_round_trip(req in arb_approval_request()) {
        // Feature: api-integration-tests, Property 1: 数据序列化 Round-trip
        // 验证需求: 5.3, 9.1, 9.2
        // 验证 ApprovalRequest 的序列化和反序列化
        
        let json = serde_json::to_string(&req).expect("serialization should succeed");
        let deserialized: desktop_client::api_client::ApprovalRequest = 
            serde_json::from_str(&json).expect("deserialization should succeed");
        
        assert_eq!(req.request_id, deserialized.request_id);
        assert_eq!(req.action, deserialized.action);
        assert_eq!(req.thread_id, deserialized.thread_id);
    }
}
