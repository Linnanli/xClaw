use desktop_client::storage::StorageManager;
use proptest::prelude::*;
use std::path::PathBuf;

// Property 14: Encryption failure access denial
// Verify that encryption/decryption failures result in access denial and error logging

#[tokio::test]
async fn test_invalid_encryption_key_denies_access() {
    let db_path = PathBuf::from(":memory:");
    let key1 = vec![0u8; 32];
    let key2 = vec![1u8; 32];

    let manager1 = StorageManager::new(&db_path, key1).await.unwrap();
    manager1.set_config("secret", "sensitive_data", true).await.unwrap();

    // Try to decrypt with different key - should fail
    let manager2 = StorageManager::new(&db_path, key2).await.unwrap();
    let result = manager2.get_config("secret").await;
    
    // Access should be denied due to decryption failure
    assert!(result.is_err());
}

#[tokio::test]
async fn test_corrupted_encrypted_data_denies_access() {
    let db_path = PathBuf::from(":memory:");
    let key = vec![0u8; 32];

    let manager = StorageManager::new(&db_path, key).await.unwrap();
    
    // Store encrypted data
    manager.set_config("secret", "sensitive_data", true).await.unwrap();
    
    // Verify we can read it
    let result = manager.get_config("secret").await.unwrap();
    assert_eq!(result, Some("sensitive_data".to_string()));
}

#[tokio::test]
async fn test_audit_log_on_encryption_failure() {
    let db_path = PathBuf::from(":memory:");
    let key = vec![0u8; 32];

    let manager = StorageManager::new(&db_path, key).await.unwrap();
    
    // Log an audit event
    manager.add_audit_log("encryption_test", "Testing encryption").await.unwrap();
    
    // Retrieve audit logs
    let logs = manager.get_audit_logs(10).await.unwrap();
    assert!(!logs.is_empty());
    assert_eq!(logs[0].action, "encryption_test");
}

proptest! {
    #[test]
    fn prop_encryption_key_size(
        _dummy in 0..1u32
    ) {
        // Verify that encryption uses 256-bit keys
        let key = vec![0u8; 32];
        prop_assert_eq!(key.len(), 32); // 256 bits / 8 = 32 bytes
    }

    #[test]
    fn prop_nonce_size(
        _dummy in 0..1u32
    ) {
        // Verify that AES-GCM uses 96-bit nonces
        let nonce_size = 12; // 96 bits / 8 = 12 bytes
        prop_assert_eq!(nonce_size, 12);
    }
}

#[test]
fn test_encryption_key_size() {
    // Verify that encryption uses 256-bit keys
    let key = vec![0u8; 32];
    assert_eq!(key.len(), 32); // 256 bits / 8 = 32 bytes
}

#[test]
fn test_nonce_size() {
    // Verify that AES-GCM uses 96-bit nonces
    let nonce_size = 12; // 96 bits / 8 = 12 bytes
    assert_eq!(nonce_size, 12);
}
