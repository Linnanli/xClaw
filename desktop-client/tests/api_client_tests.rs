use desktop_client::api_client::{ApiClient, Message, ThreadInfo};

#[test]
fn test_api_client_creation() {
    let _client = ApiClient::new("http://localhost:8000".to_string());
    // Just verify it can be created without panicking
    assert!(true);
}

#[test]
fn test_message_serialization() {
    let message = Message {
        id: "msg-1".to_string(),
        thread_id: "thread-1".to_string(),
        role: "user".to_string(),
        content: "Hello, world!".to_string(),
        created_at: "2024-01-01T00:00:00Z".to_string(),
    };

    let json = serde_json::to_string(&message).unwrap();
    let deserialized: Message = serde_json::from_str(&json).unwrap();
    
    assert_eq!(deserialized.id, "msg-1");
    assert_eq!(deserialized.role, "user");
    assert_eq!(deserialized.content, "Hello, world!");
}

#[test]
fn test_thread_info_serialization() {
    let thread = ThreadInfo {
        id: "thread-1".to_string(),
        state: "active".to_string(),
        turn_count: 5,
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-01T01:00:00Z".to_string(),
        title: Some("Test Thread".to_string()),
        thread_type: Some("chat".to_string()),
        channel: Some("web".to_string()),
    };

    let json = serde_json::to_string(&thread).unwrap();
    let deserialized: ThreadInfo = serde_json::from_str(&json).unwrap();
    
    assert_eq!(deserialized.id, "thread-1");
    assert_eq!(deserialized.turn_count, 5);
    assert_eq!(deserialized.title, Some("Test Thread".to_string()));
}

#[test]
fn test_message_with_empty_content() {
    let message = Message {
        id: "msg-2".to_string(),
        thread_id: "thread-1".to_string(),
        role: "assistant".to_string(),
        content: "".to_string(),
        created_at: "2024-01-01T00:00:00Z".to_string(),
    };

    let json = serde_json::to_string(&message).unwrap();
    let deserialized: Message = serde_json::from_str(&json).unwrap();
    
    assert_eq!(deserialized.content, "");
}

#[test]
fn test_message_with_special_characters() {
    let message = Message {
        id: "msg-3".to_string(),
        thread_id: "thread-1".to_string(),
        role: "user".to_string(),
        content: "Hello! 你好 🎉 \"quoted\" 'text'".to_string(),
        created_at: "2024-01-01T00:00:00Z".to_string(),
    };

    let json = serde_json::to_string(&message).unwrap();
    let deserialized: Message = serde_json::from_str(&json).unwrap();
    
    assert_eq!(deserialized.content, "Hello! 你好 🎉 \"quoted\" 'text'");
}
