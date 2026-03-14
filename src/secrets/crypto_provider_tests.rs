//! Comprehensive test suite for CryptoProvider trait and implementations.
//!
//! This module follows TDD principles with:
//! - Red: Test failures that drive implementation
//! - Green: Minimal implementation to pass tests
//! - Refactor: Code quality improvements
//!
//! Tests are organized by concern:
//! - happy_path: Normal operation scenarios
//! - edge_cases: Boundary conditions
//! - error_conditions: Error handling
//! - security_properties: Security-critical invariants

#[cfg(test)]
mod tests {
    use crate::secrets::crypto_provider::{CryptoAlgorithm, SecureBytes};
    use crate::secrets::default_crypto::DefaultCryptoProvider;
    use crate::secrets::gm_crypto::GMCryptoProvider;
    use crate::secrets::crypto_provider::CryptoProvider;
    use secrecy::SecretString;

    // ============================================================================
    // HAPPY PATH TESTS
    // ============================================================================

    mod happy_path {
        use super::*;

        #[test]
        fn test_default_provider_creation_with_valid_key() {
            let master_key = SecretString::from("test_master_key_32_bytes_long!!!".to_string());
            let provider = DefaultCryptoProvider::new(master_key);
            assert!(provider.is_ok());
        }

        #[test]
        fn test_default_provider_algorithm_identification() {
            let master_key = SecretString::from("test_master_key_32_bytes_long!!!".to_string());
            let provider = DefaultCryptoProvider::new(master_key).unwrap();
            assert_eq!(provider.algorithm(), CryptoAlgorithm::Default);
        }

        #[test]
        fn test_gm_provider_creation_with_valid_key() {
            let master_key = SecretString::from("test_master_key_16bytes_min".to_string());
            let provider = GMCryptoProvider::new(master_key);
            assert!(provider.is_ok());
        }

        #[test]
        fn test_gm_provider_algorithm_identification() {
            let master_key = SecretString::from("test_master_key_16bytes_min".to_string());
            let provider = GMCryptoProvider::new(master_key).unwrap();
            assert_eq!(provider.algorithm(), CryptoAlgorithm::ChinaCrypto);
        }

        #[test]
        fn test_encrypt_decrypt_roundtrip() {
            let master_key = SecretString::from("test_master_key_32_bytes_long!!!".to_string());
            let provider = DefaultCryptoProvider::new(master_key).unwrap();
            let plaintext = b"sensitive data";

            let (encrypted, salt) = provider.encrypt(plaintext).unwrap();
            let decrypted = provider.decrypt(&encrypted, &salt).unwrap();

            assert_eq!(decrypted.expose().as_bytes(), plaintext);
        }

        #[test]
        fn test_hash_produces_consistent_output() {
            let master_key = SecretString::from("test_master_key_32_bytes_long!!!".to_string());
            let provider = DefaultCryptoProvider::new(master_key).unwrap();
            let data = b"test data";

            let hash1 = provider.hash(data);
            let hash2 = provider.hash(data);

            assert_eq!(hash1, hash2);
            assert_eq!(hash1.len(), 32); // SHA256 produces 32 bytes
        }

        #[test]
        fn test_generate_salt_produces_valid_output() {
            let master_key = SecretString::from("test_master_key_32_bytes_long!!!".to_string());
            let provider = DefaultCryptoProvider::new(master_key).unwrap();

            let salt = provider.generate_salt();
            assert_eq!(salt.len(), 32);
            assert!(salt.iter().any(|&b| b != 0)); // Not all zeros
        }
    }

    // ============================================================================
    // EDGE CASES TESTS
    // ============================================================================

    mod edge_cases {
        use super::*;

        #[test]
        fn test_encrypt_empty_plaintext() {
            let master_key = SecretString::from("test_master_key_32_bytes_long!!!".to_string());
            let provider = DefaultCryptoProvider::new(master_key).unwrap();

            let (encrypted, salt) = provider.encrypt(b"").unwrap();
            let decrypted = provider.decrypt(&encrypted, &salt).unwrap();

            assert!(decrypted.is_empty());
        }

        #[test]
        fn test_encrypt_large_plaintext() {
            let master_key = SecretString::from("test_master_key_32_bytes_long!!!".to_string());
            let provider = DefaultCryptoProvider::new(master_key).unwrap();
            let large_data = vec![0x42u8; 1024 * 1024]; // 1 MB

            let (encrypted, salt) = provider.encrypt(&large_data).unwrap();
            let decrypted = provider.decrypt(&encrypted, &salt).unwrap();

            assert_eq!(decrypted.expose().as_bytes(), large_data.as_slice());
        }

        #[test]
        fn test_encrypt_unicode_plaintext() {
            let master_key = SecretString::from("test_master_key_32_bytes_long!!!".to_string());
            let provider = DefaultCryptoProvider::new(master_key).unwrap();
            let unicode_data = "Hello 世界 🔐".as_bytes();

            let (encrypted, salt) = provider.encrypt(unicode_data).unwrap();
            let decrypted = provider.decrypt(&encrypted, &salt).unwrap();

            assert_eq!(decrypted.expose(), "Hello 世界 🔐");
        }

        #[test]
        fn test_different_salts_produce_different_ciphertexts() {
            let master_key = SecretString::from("test_master_key_32_bytes_long!!!".to_string());
            let provider = DefaultCryptoProvider::new(master_key).unwrap();
            let plaintext = b"same data";

            let (encrypted1, salt1) = provider.encrypt(plaintext).unwrap();
            let (encrypted2, salt2) = provider.encrypt(plaintext).unwrap();

            assert_ne!(salt1, salt2);
            assert_ne!(encrypted1, encrypted2);

            // But both decrypt to the same value
            let decrypted1 = provider.decrypt(&encrypted1, &salt1).unwrap();
            let decrypted2 = provider.decrypt(&encrypted2, &salt2).unwrap();
            assert_eq!(decrypted1.expose(), decrypted2.expose());
        }

        #[test]
        fn test_hash_different_inputs_produce_different_hashes() {
            let master_key = SecretString::from("test_master_key_32_bytes_long!!!".to_string());
            let provider = DefaultCryptoProvider::new(master_key).unwrap();

            let hash1 = provider.hash(b"data1");
            let hash2 = provider.hash(b"data2");

            assert_ne!(hash1, hash2);
        }

        #[test]
        fn test_generate_salt_produces_unique_values() {
            let master_key = SecretString::from("test_master_key_32_bytes_long!!!".to_string());
            let provider = DefaultCryptoProvider::new(master_key).unwrap();

            let salt1 = provider.generate_salt();
            let salt2 = provider.generate_salt();
            let salt3 = provider.generate_salt();

            assert_ne!(salt1, salt2);
            assert_ne!(salt2, salt3);
            assert_ne!(salt1, salt3);
        }
    }

    // ============================================================================
    // ERROR CONDITIONS TESTS
    // ============================================================================

    mod error_conditions {
        use super::*;

        #[test]
        fn test_default_provider_rejects_short_master_key() {
            let short_key = SecretString::from("tooshort".to_string());
            let result = DefaultCryptoProvider::new(short_key);
            assert!(result.is_err());
        }

        #[test]
        fn test_gm_provider_rejects_short_master_key() {
            let short_key = SecretString::from("short".to_string());
            let result = GMCryptoProvider::new(short_key);
            assert!(result.is_err());
        }

        #[test]
        fn test_decrypt_with_wrong_salt_fails() {
            let master_key = SecretString::from("test_master_key_32_bytes_long!!!".to_string());
            let provider = DefaultCryptoProvider::new(master_key).unwrap();
            let plaintext = b"secret";

            let (encrypted, _correct_salt) = provider.encrypt(plaintext).unwrap();
            let wrong_salt = provider.generate_salt();

            let result = provider.decrypt(&encrypted, &wrong_salt);
            assert!(result.is_err());
        }

        #[test]
        fn test_decrypt_tampered_ciphertext_fails() {
            let master_key = SecretString::from("test_master_key_32_bytes_long!!!".to_string());
            let provider = DefaultCryptoProvider::new(master_key).unwrap();
            let plaintext = b"secret";

            let (mut encrypted, salt) = provider.encrypt(plaintext).unwrap();

            // Tamper with the ciphertext
            if let Some(byte) = encrypted.last_mut() {
                *byte ^= 0xFF;
            }

            let result = provider.decrypt(&encrypted, &salt);
            assert!(result.is_err());
        }

        #[test]
        fn test_gm_provider_encrypt_not_implemented() {
            let master_key = SecretString::from("test_master_key_16bytes_min".to_string());
            let provider = GMCryptoProvider::new(master_key).unwrap();

            let result = provider.encrypt(b"test");
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("not yet implemented"));
        }

        #[test]
        fn test_gm_provider_decrypt_not_implemented() {
            let master_key = SecretString::from("test_master_key_16bytes_min".to_string());
            let provider = GMCryptoProvider::new(master_key).unwrap();
            let salt = provider.generate_salt();

            let result = provider.decrypt(b"fake", &salt);
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("not yet implemented"));
        }
    }

    // ============================================================================
    // SECURITY PROPERTIES TESTS
    // ============================================================================

    mod security_properties {
        use super::*;

        #[test]
        fn test_different_master_keys_produce_different_ciphertexts() {
            let key_a = SecretString::from("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string());
            let key_b = SecretString::from("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string());

            let provider_a = DefaultCryptoProvider::new(key_a).unwrap();
            let provider_b = DefaultCryptoProvider::new(key_b).unwrap();

            let plaintext = b"shared secret";
            let (enc_a, salt_a) = provider_a.encrypt(plaintext).unwrap();
            let (enc_b, salt_b) = provider_b.encrypt(plaintext).unwrap();

            // Cross-decryption should fail
            assert!(provider_a.decrypt(&enc_b, &salt_b).is_err());
            assert!(provider_b.decrypt(&enc_a, &salt_a).is_err());
        }

        #[test]
        fn test_encrypted_output_larger_than_plaintext() {
            let master_key = SecretString::from("test_master_key_32_bytes_long!!!".to_string());
            let provider = DefaultCryptoProvider::new(master_key).unwrap();
            let plaintext = b"test";

            let (encrypted, _salt) = provider.encrypt(plaintext).unwrap();

            // Encrypted should be larger (nonce + ciphertext + tag)
            assert!(encrypted.len() > plaintext.len());
        }

        #[test]
        fn test_secure_bytes_debug_redacts_content() {
            let secure = SecureBytes::new(vec![1, 2, 3, 4, 5]);
            let debug_str = format!("{:?}", secure);

            assert!(debug_str.contains("REDACTED"));
            assert!(!debug_str.contains("1"));
            assert!(!debug_str.contains("2"));
        }

        #[test]
        fn test_secure_bytes_basic_operations() {
            let mut secure = SecureBytes::new(vec![1, 2, 3]);

            assert_eq!(secure.len(), 3);
            assert!(!secure.is_empty());
            assert_eq!(secure.as_slice(), &[1, 2, 3]);

            secure.extend_from_slice(&[4, 5]);
            assert_eq!(secure.len(), 5);
            assert_eq!(secure.as_slice(), &[1, 2, 3, 4, 5]);
        }

        #[test]
        fn test_secure_bytes_with_capacity() {
            let secure = SecureBytes::with_capacity(10);
            assert_eq!(secure.len(), 0);
            assert!(secure.is_empty());
        }

        #[test]
        fn test_secure_bytes_into_vec() {
            let secure = SecureBytes::new(vec![1, 2, 3]);
            let vec = secure.into_vec();
            assert_eq!(vec, vec![1, 2, 3]);
        }
    }

    // ============================================================================
    // PROPERTY-BASED TESTS (using proptest)
    // ============================================================================

    mod property_tests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn prop_encrypt_decrypt_roundtrip(plaintext in ".*") {
                let master_key = SecretString::from("test_master_key_32_bytes_long!!!".to_string());
                let provider = DefaultCryptoProvider::new(master_key).unwrap();

                let (encrypted, salt) = provider.encrypt(plaintext.as_bytes()).unwrap();
                let decrypted = provider.decrypt(&encrypted, &salt).unwrap();

                prop_assert_eq!(decrypted.expose(), plaintext);
            }

            #[test]
            fn prop_hash_deterministic(data in ".*") {
                let master_key = SecretString::from("test_master_key_32_bytes_long!!!".to_string());
                let provider = DefaultCryptoProvider::new(master_key).unwrap();

                let hash1 = provider.hash(data.as_bytes());
                let hash2 = provider.hash(data.as_bytes());

                prop_assert_eq!(hash1, hash2);
            }

            #[test]
            fn prop_different_plaintexts_different_ciphertexts(
                plaintext1 in ".*",
                plaintext2 in ".*"
            ) {
                prop_assume!(plaintext1 != plaintext2);

                let master_key = SecretString::from("test_master_key_32_bytes_long!!!".to_string());
                let provider = DefaultCryptoProvider::new(master_key).unwrap();

                let (encrypted1, salt1) = provider.encrypt(plaintext1.as_bytes()).unwrap();
                let (encrypted2, salt2) = provider.encrypt(plaintext2.as_bytes()).unwrap();

                // Different plaintexts should produce different ciphertexts
                // (with very high probability, though not guaranteed due to randomness)
                prop_assert_ne!(encrypted1, encrypted2);
            }
        }
    }
}
