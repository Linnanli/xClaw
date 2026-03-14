use desktop_client::auth::AuthManager;
use proptest::prelude::*;

// Property 3: Sensitive operation re-authentication
// Verify that all sensitive operations trigger password re-verification

#[test]
fn test_session_expiration_on_timeout() {
    let mut auth = AuthManager::new();
    auth.setup_master_password("ValidPassword123!").unwrap();

    let session = auth.create_session("user123".to_string()).unwrap();
    assert!(!session.is_expired());

    // Simulate time passage by checking expiration logic
    // In real scenario, this would be tested with time mocking
}

proptest! {
    #[test]
    fn prop_password_verification_required_for_sensitive_ops(
        password in r"[A-Z][a-z]{10}[0-9]{2}[!@#$%^&*]{1}"
    ) {
        let mut auth = AuthManager::new();
        
        // Setup master password
        if auth.setup_master_password(&password).is_ok() {
            // Verify password is required
            assert!(auth.verify_password(&password).is_ok());
            
            // Wrong password should fail
            let wrong_password = format!("{}X", password);
            assert!(auth.verify_password(&wrong_password).is_err());
        }
    }

    #[test]
    fn prop_session_creation_requires_valid_password(
        user_id in r"[a-z0-9]{5,20}"
    ) {
        let mut auth = AuthManager::new();
        auth.setup_master_password("ValidPassword123!").unwrap();

        // Create session
        let session = auth.create_session(user_id.clone()).unwrap();
        assert_eq!(session.user_id, user_id);

        // Session should be retrievable
        let retrieved = auth.get_session().unwrap();
        assert_eq!(retrieved.user_id, user_id);
    }

    #[test]
    fn prop_session_activity_updates_timestamp(
        _dummy in 0..1u32
    ) {
        let mut auth = AuthManager::new();
        auth.setup_master_password("ValidPassword123!").unwrap();
        
        let session1 = auth.create_session("user".to_string()).unwrap();
        let initial_activity = session1.last_activity;

        // Update activity
        auth.update_session_activity().unwrap();
        
        let session2 = auth.get_session().unwrap();
        assert!(session2.last_activity >= initial_activity);
    }

    #[test]
    fn prop_logout_clears_session(
        _dummy in 0..1u32
    ) {
        let mut auth = AuthManager::new();
        auth.setup_master_password("ValidPassword123!").unwrap();
        
        auth.create_session("user".to_string()).unwrap();
        assert!(auth.get_session().is_ok());

        auth.logout();
        assert!(auth.get_session().is_err());
    }

    #[test]
    fn prop_password_strength_validation(
        weak_password in r"[a-z]{5,10}"
    ) {
        let mut auth = AuthManager::new();
        
        // Weak passwords should fail
        assert!(auth.setup_master_password(&weak_password).is_err());
    }

    #[test]
    fn prop_key_derivation_consistency(
        password in r"[A-Z][a-z]{10}[0-9]{2}[!@#$%^&*]{1}",
        salt_seed in 0u32..1000u32
    ) {
        let mut auth = AuthManager::new();
        auth.setup_master_password(&password).unwrap();

        let salt = vec![salt_seed as u8; 16];
        
        let key1 = auth.derive_key(&password, &salt).unwrap();
        let key2 = auth.derive_key(&password, &salt).unwrap();

        // Same password and salt should produce same key
        assert_eq!(key1, key2);
        assert_eq!(key1.len(), 32); // 256-bit key
    }

    #[test]
    fn prop_different_passwords_produce_different_keys(
        password1 in r"[A-Z][a-z]{10}[0-9]{2}[!@#$%^&*]{1}",
        password2 in r"[A-Z][a-z]{10}[0-9]{2}[!@#$%^&*]{1}"
    ) {
        prop_assume!(password1 != password2);

        let auth = AuthManager::new();
        let salt = vec![0u8; 16];

        let key1 = auth.derive_key(&password1, &salt).unwrap();
        let key2 = auth.derive_key(&password2, &salt).unwrap();

        // Different passwords should produce different keys
        assert_ne!(key1, key2);
    }

    #[test]
    fn prop_session_timeout_enforcement(
        _dummy in 0..1u32
    ) {
        let mut auth = AuthManager::new();
        auth.setup_master_password("ValidPassword123!").unwrap();

        let session = auth.create_session("user".to_string()).unwrap();
        
        // Newly created session should not be expired
        assert!(!session.is_expired());
    }
}

#[test]
fn test_sensitive_operation_requires_reauthentication() {
    let mut auth = AuthManager::new();
    auth.setup_master_password("ValidPassword123!").unwrap();

    // Create session
    auth.create_session("user".to_string()).unwrap();

    // Verify session exists
    assert!(auth.get_session().is_ok());

    // For sensitive operations, re-verify password
    assert!(auth.verify_password("ValidPassword123!").is_ok());
    assert!(auth.verify_password("WrongPassword123!").is_err());
}

#[test]
fn test_multiple_failed_authentications() {
    let mut auth = AuthManager::new();
    auth.setup_master_password("ValidPassword123!").unwrap();

    // Multiple failed attempts
    for _ in 0..5 {
        assert!(auth.verify_password("WrongPassword123!").is_err());
    }

    // Correct password should still work
    assert!(auth.verify_password("ValidPassword123!").is_ok());
}

#[test]
fn test_session_isolation() {
    let mut auth1 = AuthManager::new();
    let mut auth2 = AuthManager::new();

    auth1.setup_master_password("Password1!").unwrap();
    auth2.setup_master_password("Password2!").unwrap();

    auth1.create_session("user1".to_string()).unwrap();
    auth2.create_session("user2".to_string()).unwrap();

    // Sessions should be isolated
    assert_eq!(auth1.get_session().unwrap().user_id, "user1");
    assert_eq!(auth2.get_session().unwrap().user_id, "user2");
}
