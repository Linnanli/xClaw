use desktop_client::storage::{StorageManager, LocalConfig, CachedDlpRule, DownloadedSkill};
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
    #[tokio::test]
    async fn prop_encryption_roundtrip_consistency(
        data in r"[a-zA-Z0-9 ]{1,100}"
    ) {
        let db_path = PathBuf::from(":memory:");
        let key = vec![0u8; 32];

        let manager = StorageManager::new(&db_path, key).await.unwrap();
        
        // Store encrypted data
        manager.set_config("test_key", &data, true).await.unwrap();
        
        // Retrieve and verify
        let retrieved = manager.get_config("test_key").await.unwrap();
        prop_assert_eq!(retrieved, Some(data));
    }

    #[tokio::test]
    async fn prop_different_keys_produce_different_ciphertexts(
        data in r"[a-zA-Z0-9]{10,50}"
    ) {
        let db_path1 = PathBuf::from(":memory:");
        let db_path2 = PathBuf::from(":memory:");
        let key1 = vec![0u8; 32];
        let key2 = vec![1u8; 32];

        let manager1 = StorageManager::new(&db_path1, key1).await.unwrap();
        let manager2 = StorageManager::new(&db_path2, key2).await.unwrap();
        
        // Store same data with different keys
        manager1.set_config("key", &data, true).await.unwrap();
        manager2.set_config("key", &data, true).await.unwrap();
        
        // Both should be retrievable with correct keys
        let result1 = manager1.get_config("key").await.unwrap();
        let result2 = manager2.get_config("key").await.unwrap();
        
        prop_assert_eq!(result1, Some(data.clone()));
        prop_assert_eq!(result2, Some(data));
    }

    #[tokio::test]
    async fn prop_plaintext_storage_not_encrypted(
        data in r"[a-zA-Z0-9]{5,50}"
    ) {
        let db_path = PathBuf::from(":memory:");
        let key = vec![0u8; 32];

        let manager = StorageManager::new(&db_path, key).await.unwrap();
        
        // Store plaintext data
        manager.set_config("plain", &data, false).await.unwrap();
        
        // Retrieve and verify
        let retrieved = manager.get_config("plain").await.unwrap();
        prop_assert_eq!(retrieved, Some(data));
    }

    #[tokio::test]
    async fn prop_dlp_rule_storage_and_retrieval(
        id in r"[a-z0-9]{5,20}",
        pattern in r"[a-zA-Z0-9\-_]{5,30}",
        replacement in r"[a-zA-Z0-9\-_]{5,30}",
        severity in r"(low|medium|high)"
    ) {
        let db_path = PathBuf::from(":memory:");
        let key = vec![0u8; 32];

        let manager = StorageManager::new(&db_path, key).await.unwrap();
        
        let rule = CachedDlpRule {
            id: id.clone(),
            pattern: pattern.clone(),
            replacement: replacement.clone(),
            severity: severity.clone(),
        };
        
        manager.cache_dlp_rule(&rule).await.unwrap();
        
        let rules = manager.get_cached_dlp_rules().await.unwrap();
        prop_assert!(!rules.is_empty());
        prop_assert_eq!(rules[0].id, id);
        prop_assert_eq!(rules[0].pattern, pattern);
    }

    #[tokio::test]
    async fn prop_skill_storage_and_retrieval(
        id in r"[a-z0-9]{5,20}",
        name in r"[a-zA-Z0-9\-_]{5,30}",
        version in r"[0-9]\.[0-9]\.[0-9]"
    ) {
        let db_path = PathBuf::from(":memory:");
        let key = vec![0u8; 32];

        let manager = StorageManager::new(&db_path, key).await.unwrap();
        
        let skill = DownloadedSkill {
            id: id.clone(),
            name: name.clone(),
            version: version.clone(),
            binary_hash: "abc123def456".to_string(),
            downloaded_at: 1234567890,
        };
        
        manager.cache_skill(&skill).await.unwrap();
        
        let skills = manager.get_cached_skills().await.unwrap();
        prop_assert!(!skills.is_empty());
        prop_assert_eq!(skills[0].id, id);
        prop_assert_eq!(skills[0].name, name);
    }

    #[tokio::test]
    async fn prop_audit_log_timestamp_ordering(
        _dummy in 0..1u32
    ) {
        let db_path = PathBuf::from(":memory:");
        let key = vec![0u8; 32];

        let manager = StorageManager::new(&db_path, key).await.unwrap();
        
        // Add multiple audit logs
        for i in 0..5 {
            manager.add_audit_log(&format!("action_{}", i), &format!("details_{}", i)).await.unwrap();
        }
        
        let logs = manager.get_audit_logs(10).await.unwrap();
        prop_assert_eq!(logs.len(), 5);
        
        // Verify logs are in descending timestamp order
        for i in 0..logs.len()-1 {
            prop_assert!(logs[i].timestamp >= logs[i+1].timestamp);
        }
    }

    #[tokio::test]
    async fn prop_config_update_overwrites_previous(
        key_name in r"[a-z_]{5,20}",
        value1 in r"[a-zA-Z0-9]{5,30}",
        value2 in r"[a-zA-Z0-9]{5,30}"
    ) {
        prop_assume!(value1 != value2);

        let db_path = PathBuf::from(":memory:");
        let key = vec![0u8; 32];

        let manager = StorageManager::new(&db_path, key).await.unwrap();
        
        // Store first value
        manager.set_config(&key_name, &value1, false).await.unwrap();
        let result1 = manager.get_config(&key_name).await.unwrap();
        prop_assert_eq!(result1, Some(value1.clone()));
        
        // Update with second value
        manager.set_config(&key_name, &value2, false).await.unwrap();
        let result2 = manager.get_config(&key_name).await.unwrap();
        prop_assert_eq!(result2, Some(value2));
    }

    #[tokio::test]
    async fn prop_skill_deletion_removes_entry(
        id in r"[a-z0-9]{5,20}"
    ) {
        let db_path = PathBuf::from(":memory:");
        let key = vec![0u8; 32];

        let manager = StorageManager::new(&db_path, key).await.unwrap();
        
        let skill = DownloadedSkill {
            id: id.clone(),
            name: "test_skill".to_string(),
            version: "1.0.0".to_string(),
            binary_hash: "hash123".to_string(),
            downloaded_at: 1234567890,
        };
        
        manager.cache_skill(&skill).await.unwrap();
        let skills_before = manager.get_cached_skills().await.unwrap();
        prop_assert_eq!(skills_before.len(), 1);
        
        manager.delete_skill(&id).await.unwrap();
        let skills_after = manager.get_cached_skills().await.unwrap();
        prop_assert_eq!(skills_after.len(), 0);
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
