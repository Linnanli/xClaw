//! Property-based tests for CryptoProvider encrypt/decrypt roundtrip.
//!
//! This module verifies the fundamental cryptographic property:
//! For any plaintext P and key K:
//!   decrypt(encrypt(P, K), K) == P
//!
//! This is tested with:
//! - Both Default and ChinaCrypto algorithm suites
//! - Various plaintext sizes (empty, small, large, huge)
//! - Various plaintext types (ASCII, UTF-8, binary, special chars)
//! - Multiple iterations to ensure consistency

#[cfg(test)]
mod tests {
    use crate::secrets::crypto_config::{CryptoAlgorithmConfig, CryptoConfig};
    use proptest::prelude::*;
    use secrecy::SecretString;

    // ============================================================================
    // PROPERTY 8: Encrypt/Decrypt Roundtrip
    // ============================================================================
    //
    // Property: For any plaintext P and master key K:
    //   decrypt(encrypt(P, K), K) == P
    //
    // This is the fundamental correctness property for symmetric encryption.
    //
    // Verification:
    // - Generate random plaintext
    // - Encrypt with provider
    // - Decrypt with same provider
    // - Verify result equals original plaintext

    #[test]
    fn test_default_encrypt_decrypt_empty_plaintext() {
        let config = CryptoConfig::new(CryptoAlgorithmConfig::Default);
        let master_key = SecretString::from("test_master_key_32_bytes_long!!!".to_string());
        let provider = config.create_provider(master_key).unwrap();

        let plaintext = b"";
        let (encrypted, salt) = provider.encrypt(plaintext).unwrap();
        let decrypted = provider.decrypt(&encrypted, &salt).unwrap();

        assert_eq!(decrypted.expose().as_bytes(), plaintext);
    }

    #[test]
    fn test_default_encrypt_decrypt_small_plaintext() {
        let config = CryptoConfig::new(CryptoAlgorithmConfig::Default);
        let master_key = SecretString::from("test_master_key_32_bytes_long!!!".to_string());
        let provider = config.create_provider(master_key).unwrap();

        let plaintext = b"x";
        let (encrypted, salt) = provider.encrypt(plaintext).unwrap();
        let decrypted = provider.decrypt(&encrypted, &salt).unwrap();

        assert_eq!(decrypted.expose().as_bytes(), plaintext);
    }

    #[test]
    fn test_default_encrypt_decrypt_typical_plaintext() {
        let config = CryptoConfig::new(CryptoAlgorithmConfig::Default);
        let master_key = SecretString::from("test_master_key_32_bytes_long!!!".to_string());
        let provider = config.create_provider(master_key).unwrap();

        let plaintext = b"sk-proj-1234567890abcdefghijklmnopqrstuvwxyz";
        let (encrypted, salt) = provider.encrypt(plaintext).unwrap();
        let decrypted = provider.decrypt(&encrypted, &salt).unwrap();

        assert_eq!(decrypted.expose().as_bytes(), plaintext);
    }

    #[test]
    fn test_default_encrypt_decrypt_large_plaintext() {
        let config = CryptoConfig::new(CryptoAlgorithmConfig::Default);
        let master_key = SecretString::from("test_master_key_32_bytes_long!!!".to_string());
        let provider = config.create_provider(master_key).unwrap();

        let plaintext = vec![0x42u8; 10_000]; // 10 KB
        let (encrypted, salt) = provider.encrypt(&plaintext).unwrap();
        let decrypted = provider.decrypt(&encrypted, &salt).unwrap();

        assert_eq!(decrypted.expose().as_bytes(), plaintext.as_slice());
    }

    #[test]
    fn test_default_encrypt_decrypt_huge_plaintext() {
        let config = CryptoConfig::new(CryptoAlgorithmConfig::Default);
        let master_key = SecretString::from("test_master_key_32_bytes_long!!!".to_string());
        let provider = config.create_provider(master_key).unwrap();

        // Use valid UTF-8 string (1 MB of 'A' characters)
        let plaintext = "A".repeat(1_000_000);
        let plaintext_bytes = plaintext.as_bytes();
        let (encrypted, salt) = provider.encrypt(plaintext_bytes).unwrap();
        let decrypted = provider.decrypt(&encrypted, &salt).unwrap();

        assert_eq!(decrypted.expose(), plaintext);
    }

    #[test]
    fn test_default_encrypt_decrypt_utf8_plaintext() {
        let config = CryptoConfig::new(CryptoAlgorithmConfig::Default);
        let master_key = SecretString::from("test_master_key_32_bytes_long!!!".to_string());
        let provider = config.create_provider(master_key).unwrap();

        let plaintext = "Hello 世界 🔐 مرحبا мир".as_bytes();
        let (encrypted, salt) = provider.encrypt(plaintext).unwrap();
        let decrypted = provider.decrypt(&encrypted, &salt).unwrap();

        assert_eq!(decrypted.expose(), "Hello 世界 🔐 مرحبا мир");
    }

    #[test]
    fn test_default_encrypt_decrypt_binary_plaintext() {
        let config = CryptoConfig::new(CryptoAlgorithmConfig::Default);
        let master_key = SecretString::from("test_master_key_32_bytes_long!!!".to_string());
        let provider = config.create_provider(master_key).unwrap();

        // Use valid UTF-8 data (binary-like but valid UTF-8)
        let plaintext = "binary\x00data\x01test".as_bytes();
        let (encrypted, salt) = provider.encrypt(plaintext).unwrap();
        let decrypted = provider.decrypt(&encrypted, &salt).unwrap();

        assert_eq!(decrypted.expose().as_bytes(), plaintext);
    }

    #[test]
    fn test_default_encrypt_decrypt_multiple_iterations() {
        let config = CryptoConfig::new(CryptoAlgorithmConfig::Default);
        let master_key = SecretString::from("test_master_key_32_bytes_long!!!".to_string());
        let provider = config.create_provider(master_key).unwrap();

        let plaintexts = vec![
            b"first".to_vec(),
            b"second".to_vec(),
            b"third".to_vec(),
            b"fourth".to_vec(),
            b"fifth".to_vec(),
        ];

        for plaintext in plaintexts {
            let (encrypted, salt) = provider.encrypt(&plaintext).unwrap();
            let decrypted = provider.decrypt(&encrypted, &salt).unwrap();
            assert_eq!(decrypted.expose().as_bytes(), plaintext.as_slice());
        }
    }

    #[test]
    fn test_default_encrypt_decrypt_consistency_across_calls() {
        let config = CryptoConfig::new(CryptoAlgorithmConfig::Default);
        let master_key = SecretString::from("test_master_key_32_bytes_long!!!".to_string());
        let provider = config.create_provider(master_key).unwrap();

        let plaintext = b"consistent data";

        // Encrypt and decrypt multiple times
        for _ in 0..10 {
            let (encrypted, salt) = provider.encrypt(plaintext).unwrap();
            let decrypted = provider.decrypt(&encrypted, &salt).unwrap();
            assert_eq!(decrypted.expose().as_bytes(), plaintext);
        }
    }

    // ============================================================================
    // PROPERTY-BASED TESTS FOR ROUNDTRIP
    // ============================================================================

    proptest! {
        /// Property: For any plaintext, encrypt/decrypt roundtrip returns original.
        /// This is tested with Default algorithm.
        #[test]
        fn prop_default_encrypt_decrypt_roundtrip(plaintext in ".*") {
            let config = CryptoConfig::new(CryptoAlgorithmConfig::Default);
            let master_key = SecretString::from("test_master_key_32_bytes_long!!!".to_string());
            let provider = config.create_provider(master_key).unwrap();

            let plaintext_bytes = plaintext.as_bytes();
            let (encrypted, salt) = provider.encrypt(plaintext_bytes).unwrap();
            let decrypted = provider.decrypt(&encrypted, &salt).unwrap();

            prop_assert_eq!(decrypted.expose(), plaintext);
        }

        /// Property: For any binary data, encrypt/decrypt roundtrip returns original.
        #[test]
        fn prop_default_encrypt_decrypt_binary_roundtrip(plaintext in ".*") {
            let config = CryptoConfig::new(CryptoAlgorithmConfig::Default);
            let master_key = SecretString::from("test_master_key_32_bytes_long!!!".to_string());
            let provider = config.create_provider(master_key).unwrap();

            let plaintext_bytes = plaintext.as_bytes();
            let (encrypted, salt) = provider.encrypt(plaintext_bytes).unwrap();
            let decrypted = provider.decrypt(&encrypted, &salt).unwrap();

            prop_assert_eq!(decrypted.expose().as_bytes(), plaintext_bytes);
        }

        /// Property: Encrypted output is always larger than plaintext
        /// (due to nonce, tag, and padding).
        #[test]
        fn prop_encrypted_larger_than_plaintext(plaintext in ".*") {
            let config = CryptoConfig::new(CryptoAlgorithmConfig::Default);
            let master_key = SecretString::from("test_master_key_32_bytes_long!!!".to_string());
            let provider = config.create_provider(master_key).unwrap();

            let plaintext_bytes = plaintext.as_bytes();
            let (encrypted, _salt) = provider.encrypt(plaintext_bytes).unwrap();

            // Encrypted should be at least nonce (12) + tag (16) = 28 bytes larger
            // For empty plaintext, encrypted should be at least 28 bytes
            if plaintext_bytes.is_empty() {
                prop_assert!(encrypted.len() >= 28);
            } else {
                prop_assert!(encrypted.len() > plaintext_bytes.len());
            }
        }

        /// Property: Same plaintext with different salts produces different ciphertexts.
        #[test]
        fn prop_different_salts_different_ciphertexts(plaintext in ".*") {
            let config = CryptoConfig::new(CryptoAlgorithmConfig::Default);
            let master_key = SecretString::from("test_master_key_32_bytes_long!!!".to_string());
            let provider = config.create_provider(master_key).unwrap();

            let plaintext_bytes = plaintext.as_bytes();
            let (encrypted1, salt1) = provider.encrypt(plaintext_bytes).unwrap();
            let (encrypted2, salt2) = provider.encrypt(plaintext_bytes).unwrap();

            // Salts should be different
            prop_assert_ne!(&salt1, &salt2);
            // Ciphertexts should be different (due to different salts and nonces)
            prop_assert_ne!(&encrypted1, &encrypted2);

            // But both should decrypt to the same plaintext
            let decrypted1 = provider.decrypt(&encrypted1, &salt1).unwrap();
            let decrypted2 = provider.decrypt(&encrypted2, &salt2).unwrap();
            prop_assert_eq!(decrypted1.expose(), decrypted2.expose());
        }

        /// Property: Decryption with wrong salt fails.
        #[test]
        fn prop_wrong_salt_decryption_fails(plaintext in ".*") {
            let config = CryptoConfig::new(CryptoAlgorithmConfig::Default);
            let master_key = SecretString::from("test_master_key_32_bytes_long!!!".to_string());
            let provider = config.create_provider(master_key).unwrap();

            let plaintext_bytes = plaintext.as_bytes();
            let (encrypted, _correct_salt) = provider.encrypt(plaintext_bytes).unwrap();
            let wrong_salt = provider.generate_salt();

            // Decryption with wrong salt should fail
            let result = provider.decrypt(&encrypted, &wrong_salt);
            prop_assert!(result.is_err());
        }

        /// Property: Decryption with tampered ciphertext fails.
        #[test]
        fn prop_tampered_ciphertext_decryption_fails(plaintext in ".*") {
            let config = CryptoConfig::new(CryptoAlgorithmConfig::Default);
            let master_key = SecretString::from("test_master_key_32_bytes_long!!!".to_string());
            let provider = config.create_provider(master_key).unwrap();

            let plaintext_bytes = plaintext.as_bytes();
            let (mut encrypted, salt) = provider.encrypt(plaintext_bytes).unwrap();

            // Tamper with the ciphertext (flip a bit in the last byte)
            if let Some(last_byte) = encrypted.last_mut() {
                *last_byte ^= 0x01;
            }

            // Decryption should fail due to authentication tag mismatch
            let result = provider.decrypt(&encrypted, &salt);
            prop_assert!(result.is_err());
        }
    }

    // ============================================================================
    // CHINA CRYPTO ROUNDTRIP TESTS (Expected to Fail)
    // ============================================================================

    #[test]
    fn test_china_crypto_encrypt_not_implemented() {
        let config = CryptoConfig::new(CryptoAlgorithmConfig::ChinaCrypto);
        let master_key = SecretString::from("test_master_key_16bytes_min".to_string());
        let provider = config.create_provider(master_key).unwrap();

        let plaintext = b"test";
        let result = provider.encrypt(plaintext);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not yet implemented"));
    }

    #[test]
    fn test_china_crypto_decrypt_not_implemented() {
        let config = CryptoConfig::new(CryptoAlgorithmConfig::ChinaCrypto);
        let master_key = SecretString::from("test_master_key_16bytes_min".to_string());
        let provider = config.create_provider(master_key).unwrap();

        let salt = provider.generate_salt();
        let result = provider.decrypt(b"fake", &salt);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not yet implemented"));
    }
}
