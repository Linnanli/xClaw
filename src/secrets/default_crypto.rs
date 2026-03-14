//! Default cryptographic provider using standard algorithms.
//!
//! Uses:
//! - AES-256-GCM for authenticated encryption
//! - Ed25519 for digital signatures
//! - SHA256 for hashing
//! - HKDF-SHA256 for key derivation
//!
//! # Security Properties
//!
//! - All operations are constant-time where applicable
//! - Private keys are never logged or exposed
//! - Encryption includes authentication (GCM mode)
//! - Key derivation uses industry-standard HKDF
//!
//! # Panics
//!
//! This implementation never panics in production code paths.
//! All errors are returned as Result types.

use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::{Aead, AeadCore, OsRng},
};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use hkdf::Hkdf;
use rand::RngCore;
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};

use crate::secrets::crypto_provider::{CryptoAlgorithm, CryptoProvider, SecureBytes};
use crate::secrets::types::{DecryptedSecret, SecretError};

/// Size of the AES-256 key in bytes.
const KEY_SIZE: usize = 32;

/// Size of the GCM nonce in bytes.
const NONCE_SIZE: usize = 12;

/// Size of the per-secret salt for key derivation.
const SALT_SIZE: usize = 32;

/// Size of the GCM authentication tag.
const TAG_SIZE: usize = 16;

/// Default cryptographic provider using standard algorithms.
///
/// This provider uses:
/// - AES-256-GCM for encryption
/// - Ed25519 for signatures
/// - SHA256 for hashing
/// - HKDF-SHA256 for key derivation
pub struct DefaultCryptoProvider {
    master_key: SecretString,
}

impl DefaultCryptoProvider {
    /// Create a new default crypto provider from a master key.
    ///
    /// The master key should be at least 32 bytes of high-entropy data.
    pub fn new(master_key: SecretString) -> Result<Self, SecretError> {
        // Validate master key length
        if master_key.expose_secret().len() < KEY_SIZE {
            return Err(SecretError::InvalidMasterKey);
        }
        Ok(Self { master_key })
    }

    /// Derive a per-secret key using HKDF-SHA256.
    fn derive_encryption_key(&self, salt: &[u8]) -> Result<SecureBytes, SecretError> {
        let master_bytes = self.master_key.expose_secret().as_bytes();

        // HKDF extract + expand
        let hk = Hkdf::<Sha256>::new(Some(salt), master_bytes);

        let mut derived = SecureBytes::with_capacity(KEY_SIZE);
        let mut key_bytes = vec![0u8; KEY_SIZE];
        hk.expand(b"near-agent-secrets-v1", &mut key_bytes)
            .map_err(|_| SecretError::EncryptionFailed("HKDF expansion failed".to_string()))?;

        derived.extend_from_slice(&key_bytes);

        // Zero out the temporary buffer
        key_bytes.iter_mut().for_each(|b| *b = 0);

        Ok(derived)
    }
}

impl CryptoProvider for DefaultCryptoProvider {
    fn algorithm(&self) -> CryptoAlgorithm {
        CryptoAlgorithm::Default
    }

    fn encrypt(&self, plaintext: &[u8]) -> Result<(Vec<u8>, Vec<u8>), SecretError> {
        let salt = self.generate_salt();
        let derived_key = self.derive_encryption_key(&salt)?;

        let cipher = Aes256Gcm::new_from_slice(derived_key.as_slice()).map_err(|e| {
            SecretError::EncryptionFailed(format!("Failed to create cipher: {}", e))
        })?;

        // Generate random nonce
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

        // Encrypt
        let ciphertext = cipher
            .encrypt(&nonce, plaintext)
            .map_err(|e| SecretError::EncryptionFailed(format!("Encryption failed: {}", e)))?;

        // Combine: nonce || ciphertext (which includes tag)
        let mut encrypted = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
        encrypted.extend_from_slice(&nonce);
        encrypted.extend_from_slice(&ciphertext);

        Ok((encrypted, salt))
    }

    fn decrypt(&self, encrypted_value: &[u8], salt: &[u8]) -> Result<DecryptedSecret, SecretError> {
        if encrypted_value.len() < NONCE_SIZE + TAG_SIZE {
            return Err(SecretError::DecryptionFailed(
                "Encrypted value too short".to_string(),
            ));
        }

        let derived_key = self.derive_encryption_key(salt)?;

        let cipher = Aes256Gcm::new_from_slice(derived_key.as_slice()).map_err(|e| {
            SecretError::DecryptionFailed(format!("Failed to create cipher: {}", e))
        })?;

        // Split: nonce || ciphertext
        let (nonce_bytes, ciphertext) = encrypted_value.split_at(NONCE_SIZE);
        let nonce = Nonce::from_slice(nonce_bytes);

        // Decrypt
        let plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| SecretError::DecryptionFailed(format!("Decryption failed: {}", e)))?;

        DecryptedSecret::from_bytes(plaintext)
    }

    fn sign(&self, data: &[u8], private_key: &[u8]) -> Result<Vec<u8>, SecretError> {
        if private_key.len() != 32 {
            return Err(SecretError::EncryptionFailed(
                "Ed25519 private key must be 32 bytes".to_string(),
            ));
        }

        let signing_key = SigningKey::from_bytes(private_key.try_into().map_err(|_| {
            SecretError::EncryptionFailed("Invalid private key format".to_string())
        })?);

        let signature = signing_key.sign(data);
        Ok(signature.to_bytes().to_vec())
    }

    fn verify(
        &self,
        data: &[u8],
        signature: &[u8],
        public_key: &[u8],
    ) -> Result<bool, SecretError> {
        if public_key.len() != 32 {
            return Err(SecretError::DecryptionFailed(
                "Ed25519 public key must be 32 bytes".to_string(),
            ));
        }

        let verifying_key =
            VerifyingKey::from_bytes(public_key.try_into().map_err(|_| {
                SecretError::DecryptionFailed("Invalid public key format".to_string())
            })?)
            .map_err(|e| SecretError::DecryptionFailed(format!("Invalid public key: {}", e)))?;

        let sig =
            Signature::from_bytes(signature.try_into().map_err(|_| {
                SecretError::DecryptionFailed("Invalid signature format".to_string())
            })?);

        Ok(verifying_key.verify(data, &sig).is_ok())
    }

    fn hash(&self, data: &[u8]) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher.finalize().to_vec()
    }

    fn derive_key(
        &self,
        master_key: &[u8],
        salt: &[u8],
        info: &[u8],
    ) -> Result<Vec<u8>, SecretError> {
        let hk = Hkdf::<Sha256>::new(Some(salt), master_key);
        let mut derived = vec![0u8; KEY_SIZE];
        hk.expand(info, &mut derived)
            .map_err(|_| SecretError::EncryptionFailed("HKDF expansion failed".to_string()))?;
        Ok(derived)
    }

    fn generate_salt(&self) -> Vec<u8> {
        let mut salt = vec![0u8; SALT_SIZE];
        rand::RngCore::fill_bytes(&mut OsRng, &mut salt);
        salt
    }
}

impl std::fmt::Debug for DefaultCryptoProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DefaultCryptoProvider")
            .field("algorithm", &self.algorithm())
            .field("master_key", &"[REDACTED]")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::credentials::TEST_CRYPTO_KEY;
    use secrecy::SecretString;

    fn test_provider() -> DefaultCryptoProvider {
        DefaultCryptoProvider::new(SecretString::from(TEST_CRYPTO_KEY.to_string())).unwrap()
    }

    #[test]
    fn test_algorithm() {
        let provider = test_provider();
        assert_eq!(provider.algorithm(), CryptoAlgorithm::Default);
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let provider = test_provider();
        let plaintext = b"my_super_secret_api_key_12345";

        let (encrypted, salt) = provider.encrypt(plaintext).unwrap();
        assert!(encrypted.len() > plaintext.len());

        let decrypted = provider.decrypt(&encrypted, &salt).unwrap();
        assert_eq!(decrypted.expose().as_bytes(), plaintext);
    }

    #[test]
    fn test_different_salts_different_ciphertext() {
        let provider = test_provider();
        let plaintext = b"same_secret";

        let (encrypted1, salt1) = provider.encrypt(plaintext).unwrap();
        let (encrypted2, salt2) = provider.encrypt(plaintext).unwrap();

        assert_ne!(salt1, salt2);
        assert_ne!(encrypted1, encrypted2);

        let decrypted1 = provider.decrypt(&encrypted1, &salt1).unwrap();
        let decrypted2 = provider.decrypt(&encrypted2, &salt2).unwrap();
        assert_eq!(decrypted1.expose(), decrypted2.expose());
    }

    #[test]
    fn test_sign_verify() {
        let provider = test_provider();
        let data = b"message to sign";

        // Generate a test key pair
        let mut seed = [0u8; 32];
        OsRng.fill_bytes(&mut seed);
        let signing_key = SigningKey::from_bytes(&seed);
        let private_key = signing_key.to_bytes();
        let public_key = signing_key.verifying_key().to_bytes();

        // Sign
        let signature = provider.sign(data, &private_key).unwrap();
        assert_eq!(signature.len(), 64); // Ed25519 signature is 64 bytes

        // Verify
        let valid = provider.verify(data, &signature, &public_key).unwrap();
        assert!(valid);

        // Verify with wrong data
        let wrong_data = b"different message";
        let invalid = provider
            .verify(wrong_data, &signature, &public_key)
            .unwrap();
        assert!(!invalid);
    }

    #[test]
    fn test_hash() {
        let provider = test_provider();
        let data = b"data to hash";

        let hash1 = provider.hash(data);
        let hash2 = provider.hash(data);

        assert_eq!(hash1.len(), 32); // SHA256 produces 32 bytes
        assert_eq!(hash1, hash2); // Same input produces same hash

        let different_data = b"different data";
        let hash3 = provider.hash(different_data);
        assert_ne!(hash1, hash3); // Different input produces different hash
    }

    #[test]
    fn test_derive_key() {
        let provider = test_provider();
        let master = b"master_key_material_here_32bytes";
        let salt = provider.generate_salt();
        let info = b"application-specific-context";

        let key1 = provider.derive_key(master, &salt, info).unwrap();
        let key2 = provider.derive_key(master, &salt, info).unwrap();

        assert_eq!(key1.len(), KEY_SIZE);
        assert_eq!(key1, key2); // Same inputs produce same key

        let different_salt = provider.generate_salt();
        let key3 = provider.derive_key(master, &different_salt, info).unwrap();
        assert_ne!(key1, key3); // Different salt produces different key
    }

    #[test]
    fn test_generate_salt() {
        let provider = test_provider();
        let salt1 = provider.generate_salt();
        let salt2 = provider.generate_salt();

        assert_eq!(salt1.len(), SALT_SIZE);
        assert_eq!(salt2.len(), SALT_SIZE);
        assert_ne!(salt1, salt2); // Salts should be unique
    }

    #[test]
    fn test_master_key_too_short() {
        let short_key = "tooshort";
        let result = DefaultCryptoProvider::new(SecretString::from(short_key.to_string()));
        assert!(result.is_err());
    }

    #[test]
    fn test_debug_redacts_master_key() {
        let provider = test_provider();
        let debug = format!("{:?}", provider);
        assert!(debug.contains("REDACTED"));
        assert!(debug.contains("Default"));
    }
}
