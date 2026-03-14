//! Property-based tests for CryptoProvider algorithm consistency.
//!
//! This module verifies that:
//! 1. Algorithm configuration is correctly applied
//! 2. Both Default and ChinaCrypto modes work consistently
//! 3. Algorithm switching doesn't break functionality
//! 4. Cross-algorithm operations fail appropriately

#[cfg(test)]
mod tests {
    use crate::secrets::crypto_config::{CryptoAlgorithmConfig, CryptoConfig};
    use crate::secrets::crypto_provider::CryptoAlgorithm;
    use proptest::prelude::*;
    use secrecy::SecretString;

    // ============================================================================
    // PROPERTY 1: Algorithm Configuration Consistency
    // ============================================================================
    //
    // Property: When a CryptoConfig is created with a specific algorithm,
    // the resulting provider must report that algorithm.
    //
    // Verification:
    // - Create config with Default algorithm
    // - Create provider from config
    // - Verify provider.algorithm() == CryptoAlgorithm::Default
    // - Repeat for ChinaCrypto

    #[test]
    fn test_default_algorithm_configuration_consistency() {
        let config = CryptoConfig::new(CryptoAlgorithmConfig::Default);
        let master_key = SecretString::from("test_master_key_32_bytes_long!!!".to_string());

        let provider = config.create_provider(master_key).unwrap();
        assert_eq!(provider.algorithm(), CryptoAlgorithm::Default);
    }

    #[test]
    fn test_china_crypto_algorithm_configuration_consistency() {
        let config = CryptoConfig::new(CryptoAlgorithmConfig::ChinaCrypto);
        let master_key = SecretString::from("test_master_key_16bytes_min".to_string());

        let provider = config.create_provider(master_key).unwrap();
        assert_eq!(provider.algorithm(), CryptoAlgorithm::ChinaCrypto);
    }

    #[test]
    fn test_default_config_uses_default_algorithm() {
        let config = CryptoConfig::default();
        assert_eq!(config.algorithm, CryptoAlgorithmConfig::Default);

        let master_key = SecretString::from("test_master_key_32_bytes_long!!!".to_string());
        let provider = config.create_provider(master_key).unwrap();
        assert_eq!(provider.algorithm(), CryptoAlgorithm::Default);
    }

    // ============================================================================
    // PROPERTY 2: Algorithm Switching Functionality
    // ============================================================================
    //
    // Property: Switching between algorithms should not affect the ability
    // to perform cryptographic operations (though results differ).
    //
    // Verification:
    // - Create two configs with different algorithms
    // - Create providers from both
    // - Verify both can perform operations (even if one fails with NotImplemented)
    // - Verify algorithm() returns correct value for each

    #[test]
    fn test_algorithm_switching_preserves_provider_interface() {
        let default_config = CryptoConfig::new(CryptoAlgorithmConfig::Default);
        let china_config = CryptoConfig::new(CryptoAlgorithmConfig::ChinaCrypto);

        let default_key = SecretString::from("test_master_key_32_bytes_long!!!".to_string());
        let china_key = SecretString::from("test_master_key_16bytes_min".to_string());

        let default_provider = default_config.create_provider(default_key).unwrap();
        let china_provider = china_config.create_provider(china_key).unwrap();

        // Both should report their respective algorithms
        assert_eq!(default_provider.algorithm(), CryptoAlgorithm::Default);
        assert_eq!(china_provider.algorithm(), CryptoAlgorithm::ChinaCrypto);

        // Both should be able to generate salt (algorithm-independent)
        let salt1 = default_provider.generate_salt();
        let salt2 = china_provider.generate_salt();
        assert_eq!(salt1.len(), 32);
        assert_eq!(salt2.len(), 32);
        assert_ne!(salt1, salt2);
    }

    // ============================================================================
    // PROPERTY 3: Cross-Algorithm Isolation
    // ============================================================================
    //
    // Property: Data encrypted with one algorithm cannot be decrypted
    // with another algorithm (or fails gracefully).
    //
    // Verification:
    // - Encrypt with Default algorithm
    // - Try to decrypt with ChinaCrypto algorithm
    // - Verify decryption fails

    #[test]
    fn test_cross_algorithm_decryption_fails() {
        let default_config = CryptoConfig::new(CryptoAlgorithmConfig::Default);
        let china_config = CryptoConfig::new(CryptoAlgorithmConfig::ChinaCrypto);

        let default_key = SecretString::from("test_master_key_32_bytes_long!!!".to_string());
        let china_key = SecretString::from("test_master_key_16bytes_min".to_string());

        let default_provider = default_config.create_provider(default_key).unwrap();
        let china_provider = china_config.create_provider(china_key).unwrap();

        let plaintext = b"secret data";

        // Encrypt with Default
        let (encrypted, salt) = default_provider.encrypt(plaintext).unwrap();

        // Try to decrypt with ChinaCrypto (should fail)
        let result = china_provider.decrypt(&encrypted, &salt);
        assert!(result.is_err());
    }

    // ============================================================================
    // PROPERTY 4: Algorithm Consistency Across Operations
    // ============================================================================
    //
    // Property: All operations from a provider must use the same algorithm.
    //
    // Verification:
    // - Create provider with specific algorithm
    // - Perform multiple operations (encrypt, hash, derive_key)
    // - Verify all operations complete without mixing algorithms

    #[test]
    fn test_default_provider_consistent_algorithm_usage() {
        let config = CryptoConfig::new(CryptoAlgorithmConfig::Default);
        let master_key = SecretString::from("test_master_key_32_bytes_long!!!".to_string());
        let provider = config.create_provider(master_key).unwrap();

        // All operations should work with Default algorithm
        let plaintext = b"test data";
        let (encrypted, salt) = provider.encrypt(plaintext).unwrap();
        let decrypted = provider.decrypt(&encrypted, &salt).unwrap();
        let hash = provider.hash(plaintext);
        let derived = provider.derive_key(b"master", &salt, b"info").unwrap();

        assert_eq!(decrypted.expose().as_bytes(), plaintext);
        assert_eq!(hash.len(), 32); // SHA256
        assert_eq!(derived.len(), 32); // AES-256 key size
    }

    #[test]
    fn test_china_crypto_provider_consistent_algorithm_usage() {
        let config = CryptoConfig::new(CryptoAlgorithmConfig::ChinaCrypto);
        let master_key = SecretString::from("test_master_key_16bytes_min".to_string());
        let provider = config.create_provider(master_key).unwrap();

        // Salt generation should work (algorithm-independent)
        let salt = provider.generate_salt();
        assert_eq!(salt.len(), 32);

        // Other operations should fail with NotImplemented (not panic)
        let plaintext = b"test data";
        assert!(provider.encrypt(plaintext).is_err());
        assert!(provider.decrypt(b"fake", &salt).is_err());
        assert!(provider.derive_key(b"master", &salt, b"info").is_err());
    }

    // ============================================================================
    // PROPERTY 5: Configuration Serialization Consistency
    // ============================================================================
    //
    // Property: CryptoConfig must serialize and deserialize consistently,
    // preserving the algorithm choice.
    //
    // Verification:
    // - Create config with specific algorithm
    // - Serialize to JSON
    // - Deserialize from JSON
    // - Verify algorithm is preserved

    #[test]
    fn test_config_serialization_preserves_algorithm() {
        let original = CryptoConfig::new(CryptoAlgorithmConfig::ChinaCrypto);
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: CryptoConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(original.algorithm, deserialized.algorithm);
    }

    #[test]
    fn test_config_roundtrip_consistency() {
        for algo in &[
            CryptoAlgorithmConfig::Default,
            CryptoAlgorithmConfig::ChinaCrypto,
        ] {
            let config1 = CryptoConfig::new(*algo);
            let json = serde_json::to_string(&config1).unwrap();
            let config2: CryptoConfig = serde_json::from_str(&json).unwrap();
            let json2 = serde_json::to_string(&config2).unwrap();

            assert_eq!(json, json2);
        }
    }

    // ============================================================================
    // PROPERTY-BASED TESTS
    // ============================================================================

    proptest! {
        /// Property: For any plaintext, encrypting and decrypting with
        /// the same provider returns the original plaintext.
        #[test]
        fn prop_default_encrypt_decrypt_roundtrip(plaintext in ".*") {
            let config = CryptoConfig::new(CryptoAlgorithmConfig::Default);
            let master_key = SecretString::from("test_master_key_32_bytes_long!!!".to_string());
            let provider = config.create_provider(master_key).unwrap();

            let (encrypted, salt) = provider.encrypt(plaintext.as_bytes()).unwrap();
            let decrypted = provider.decrypt(&encrypted, &salt).unwrap();

            prop_assert_eq!(decrypted.expose(), plaintext);
        }

        /// Property: For any data, hashing is deterministic.
        #[test]
        fn prop_hash_deterministic(data in ".*") {
            let config = CryptoConfig::new(CryptoAlgorithmConfig::Default);
            let master_key = SecretString::from("test_master_key_32_bytes_long!!!".to_string());
            let provider = config.create_provider(master_key).unwrap();

            let hash1 = provider.hash(data.as_bytes());
            let hash2 = provider.hash(data.as_bytes());

            prop_assert_eq!(hash1, hash2);
        }

        /// Property: Different plaintexts produce different ciphertexts
        /// (with very high probability).
        #[test]
        fn prop_different_plaintexts_different_ciphertexts(
            plaintext1 in ".*",
            plaintext2 in ".*"
        ) {
            prop_assume!(plaintext1 != plaintext2);

            let config = CryptoConfig::new(CryptoAlgorithmConfig::Default);
            let master_key = SecretString::from("test_master_key_32_bytes_long!!!".to_string());
            let provider = config.create_provider(master_key).unwrap();

            let (encrypted1, _salt1) = provider.encrypt(plaintext1.as_bytes()).unwrap();
            let (encrypted2, _salt2) = provider.encrypt(plaintext2.as_bytes()).unwrap();

            // Different plaintexts should produce different ciphertexts
            prop_assert_ne!(encrypted1, encrypted2);
        }

        /// Property: Algorithm configuration is correctly applied
        /// regardless of input.
        #[test]
        fn prop_algorithm_configuration_applied(_data in ".*") {
            let default_config = CryptoConfig::new(CryptoAlgorithmConfig::Default);
            let china_config = CryptoConfig::new(CryptoAlgorithmConfig::ChinaCrypto);

            let default_key = SecretString::from("test_master_key_32_bytes_long!!!".to_string());
            let china_key = SecretString::from("test_master_key_16bytes_min".to_string());

            let default_provider = default_config.create_provider(default_key).unwrap();
            let china_provider = china_config.create_provider(china_key).unwrap();

            prop_assert_eq!(default_provider.algorithm(), CryptoAlgorithm::Default);
            prop_assert_eq!(china_provider.algorithm(), CryptoAlgorithm::ChinaCrypto);
        }
    }
}
