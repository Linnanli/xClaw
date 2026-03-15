use admin_backend::auth::AuthManager;
use proptest::prelude::*;

prop_compose! {
    fn arb_password()(s in "[a-zA-Z0-9!@#$%^&*]{8,32}") -> String {
        s
    }
}

prop_compose! {
    fn arb_user_id()(s in "[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}") -> String {
        s
    }
}

proptest! {
    #[test]
    fn prop_hash_password_produces_valid_hash(password in arb_password()) {
        let auth = AuthManager::new("test-secret".to_string());
        let hash = auth.hash_password(&password).unwrap();
        
        prop_assert!(!hash.is_empty());
        prop_assert!(hash.contains("$argon2"));
    }

    #[test]
    fn prop_verify_password_succeeds_with_correct_password(password in arb_password()) {
        let auth = AuthManager::new("test-secret".to_string());
        let hash = auth.hash_password(&password).unwrap();
        
        prop_assert!(auth.verify_password(&password, &hash).is_ok());
    }

    #[test]
    fn prop_verify_password_fails_with_wrong_password(
        password in arb_password(),
        wrong_password in arb_password()
    ) {
        prop_assume!(password != wrong_password);
        
        let auth = AuthManager::new("test-secret".to_string());
        let hash = auth.hash_password(&password).unwrap();
        
        prop_assert!(auth.verify_password(&wrong_password, &hash).is_err());
    }

    #[test]
    fn prop_generate_access_token_creates_valid_token(user_id in arb_user_id()) {
        let auth = AuthManager::new("test-secret".to_string());
        let token = auth.generate_access_token(&user_id).unwrap();
        
        prop_assert!(!token.is_empty());
        prop_assert!(token.contains('.'));
    }

    #[test]
    fn prop_verify_token_succeeds_with_valid_token(user_id in arb_user_id()) {
        let auth = AuthManager::new("test-secret".to_string());
        let token = auth.generate_access_token(&user_id).unwrap();
        
        let claims = auth.verify_token(&token).unwrap();
        prop_assert_eq!(claims.sub, user_id);
        prop_assert_eq!(claims.token_type, "access");
    }

    #[test]
    fn prop_verify_token_fails_with_invalid_token(invalid_token in "[a-zA-Z0-9.]{20,100}") {
        let auth = AuthManager::new("test-secret".to_string());
        
        prop_assert!(auth.verify_token(&invalid_token).is_err());
    }

    #[test]
    fn prop_refresh_token_has_longer_expiry(user_id in arb_user_id()) {
        let auth = AuthManager::new("test-secret".to_string());
        
        let access_token = auth.generate_access_token(&user_id).unwrap();
        let refresh_token = auth.generate_refresh_token(&user_id).unwrap();
        
        let access_claims = auth.verify_token(&access_token).unwrap();
        let refresh_claims = auth.verify_token(&refresh_token).unwrap();
        
        prop_assert!(refresh_claims.exp > access_claims.exp);
    }

    #[test]
    fn prop_different_secrets_produce_different_tokens(
        user_id in arb_user_id(),
        secret1 in "[a-zA-Z0-9]{10,20}",
        secret2 in "[a-zA-Z0-9]{10,20}"
    ) {
        prop_assume!(secret1 != secret2);
        
        let auth1 = AuthManager::new(secret1);
        let auth2 = AuthManager::new(secret2);
        
        let token1 = auth1.generate_access_token(&user_id).unwrap();
        let token2 = auth2.generate_access_token(&user_id).unwrap();
        
        prop_assert_ne!(token1, token2);
    }

    #[test]
    fn prop_token_from_different_secret_fails_verification(
        user_id in arb_user_id(),
        secret1 in "[a-zA-Z0-9]{10,20}",
        secret2 in "[a-zA-Z0-9]{10,20}"
    ) {
        prop_assume!(secret1 != secret2);
        
        let auth1 = AuthManager::new(secret1);
        let auth2 = AuthManager::new(secret2);
        
        let token = auth1.generate_access_token(&user_id).unwrap();
        
        prop_assert!(auth2.verify_token(&token).is_err());
    }
}
