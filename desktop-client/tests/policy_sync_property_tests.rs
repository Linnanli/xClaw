use desktop_client::dlp_integration::DlpIntegration;
use desktop_client::policy_sync::{DlpPolicy, PolicySyncManager, PolicyVersion, SensitiveOpPolicy};
use proptest::prelude::*;

// Property tests for policy sync and DLP integration

#[test]
fn test_policy_version_initialization() {
    let manager = PolicySyncManager::new();
    let version = manager.get_version();

    assert_eq!(version.dlp_rules_version, 0);
    assert_eq!(version.sensitive_ops_version, 0);
}

#[test]
fn test_needs_sync_detection() {
    let manager = PolicySyncManager::new();

    let remote_v1 = PolicyVersion {
        dlp_rules_version: 1,
        sensitive_ops_version: 0,
        last_sync: 0,
    };

    let remote_v2 = PolicyVersion {
        dlp_rules_version: 0,
        sensitive_ops_version: 1,
        last_sync: 0,
    };

    let remote_same = PolicyVersion {
        dlp_rules_version: 0,
        sensitive_ops_version: 0,
        last_sync: 0,
    };

    assert!(manager.needs_sync(&remote_v1));
    assert!(manager.needs_sync(&remote_v2));
    assert!(!manager.needs_sync(&remote_same));
}

proptest! {
    #[test]
    fn prop_dlp_policy_update(
        id in r"[a-z0-9]{5,20}",
        pattern in r"[a-zA-Z0-9\-_]{5,30}",
        replacement in r"[a-zA-Z0-9\-_]{5,30}",
        severity in r"(low|medium|high)"
    ) {
        let mut manager = PolicySyncManager::new();
        let policies = vec![DlpPolicy {
            id: id.clone(),
            pattern: pattern.clone(),
            replacement: replacement.clone(),
            severity: severity.clone(),
        }];

        manager.update_dlp_policies(policies, 1).unwrap();

        let retrieved = manager.get_dlp_policies();
        prop_assert_eq!(retrieved.len(), 1);
        prop_assert_eq!(&retrieved[0].id, &id);
        prop_assert_eq!(&retrieved[0].pattern, &pattern);
    }

    #[test]
    fn prop_sensitive_ops_policy_update(
        id in r"[a-z0-9]{5,20}",
        operation in r"[a-z_]{5,30}",
        risk_level in r"(low|medium|high)"
    ) {
        let mut manager = PolicySyncManager::new();
        let policies = vec![SensitiveOpPolicy {
            id: id.clone(),
            operation: operation.clone(),
            requires_approval: true,
            risk_level: risk_level.clone(),
        }];

        manager.update_sensitive_ops_policies(policies, 1).unwrap();

        let retrieved = manager.get_sensitive_ops_policies();
        prop_assert_eq!(retrieved.len(), 1);
        prop_assert_eq!(&retrieved[0].operation, &operation);
        prop_assert!(retrieved[0].requires_approval);
    }

    #[test]
    fn prop_dlp_policy_application(
        text in r"[a-zA-Z0-9 ]{10,100}"
    ) {
        let mut manager = PolicySyncManager::new();
        let policies = vec![DlpPolicy {
            id: "test".to_string(),
            pattern: r"[0-9]{3}-[0-9]{2}-[0-9]{4}".to_string(),
            replacement: "[REDACTED]".to_string(),
            severity: "high".to_string(),
        }];

        manager.update_dlp_policies(policies, 1).unwrap();

        let result = manager.apply_dlp_policy(&text);
        // Result should be a string (may or may not contain [REDACTED])
        prop_assert!(!result.is_empty());
    }

    #[test]
    fn prop_sensitive_operation_lookup(
        operation in r"[a-z_]{5,30}"
    ) {
        let mut manager = PolicySyncManager::new();
        let policies = vec![SensitiveOpPolicy {
            id: "1".to_string(),
            operation: operation.clone(),
            requires_approval: true,
            risk_level: "high".to_string(),
        }];

        manager.update_sensitive_ops_policies(policies, 1).unwrap();

        let found = manager.get_sensitive_ops_policies()
            .iter()
            .find(|p| p.operation == operation)
            .cloned();
        prop_assert!(found.is_some());
        prop_assert_eq!(&found.unwrap().operation, &operation);
    }

    #[test]
    fn prop_version_update_increments(
        version1 in 1u64..100u64,
        version2 in 1u64..100u64
    ) {
        let mut manager = PolicySyncManager::new();
        let policies = vec![];

        manager.update_dlp_policies(policies.clone(), version1).unwrap();
        let v1 = manager.get_version().dlp_rules_version;
        prop_assert_eq!(v1, version1);

        manager.update_dlp_policies(policies, version2).unwrap();
        let v2 = manager.get_version().dlp_rules_version;
        prop_assert_eq!(v2, version2);
    }
}

#[tokio::test]
async fn test_dlp_integration_scan() {
    let mut policy_manager = PolicySyncManager::new();
    let policies = vec![DlpPolicy {
        id: "ssn".to_string(),
        pattern: r"\d{3}-\d{2}-\d{4}".to_string(),
        replacement: "[REDACTED]".to_string(),
        severity: "high".to_string(),
    }];

    policy_manager.update_dlp_policies(policies, 1).unwrap();
    let dlp = DlpIntegration::new(policy_manager);

    let result = dlp.scan_user_input("SSN: 123-45-6789").await.unwrap();
    assert!(!result.is_clean);
    assert!(!result.matches.is_empty());
}

#[tokio::test]
async fn test_dlp_integration_clean_input() {
    let dlp = DlpIntegration::default();
    let result = dlp.scan_user_input("Hello world").await.unwrap();
    assert!(result.is_clean);
}

#[tokio::test]
async fn test_dlp_outbound_request_scan() {
    let mut policy_manager = PolicySyncManager::new();
    let policies = vec![DlpPolicy {
        id: "api_key".to_string(),
        pattern: r"sk_[a-zA-Z0-9]{32}".to_string(),
        replacement: "[API_KEY]".to_string(),
        severity: "critical".to_string(),
    }];

    policy_manager.update_dlp_policies(policies, 1).unwrap();
    let dlp = DlpIntegration::new(policy_manager);

    let request = r#"{"api_key": "sk_test1234567890abcdefghijklmnopqr"}"#;
    let result = dlp.scan_outbound_request(request).await.unwrap();
    assert!(!result.is_clean);
}

#[test]
fn test_multiple_dlp_policies() {
    let mut manager = PolicySyncManager::new();
    let policies = vec![
        DlpPolicy {
            id: "ssn".to_string(),
            pattern: r"\d{3}-\d{2}-\d{4}".to_string(),
            replacement: "[SSN]".to_string(),
            severity: "high".to_string(),
        },
        DlpPolicy {
            id: "email".to_string(),
            pattern: r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}".to_string(),
            replacement: "[EMAIL]".to_string(),
            severity: "medium".to_string(),
        },
    ];

    manager.update_dlp_policies(policies, 1).unwrap();
    let retrieved = manager.get_dlp_policies();
    assert_eq!(retrieved.len(), 2);
}

#[test]
fn test_policy_sync_timestamp() {
    let mut manager = PolicySyncManager::new();
    let policies = vec![];

    let before = manager.get_version().last_sync;
    manager.update_dlp_policies(policies, 1).unwrap();
    let after = manager.get_version().last_sync;

    assert!(after >= before);
}
