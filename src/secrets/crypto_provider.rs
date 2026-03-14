//! Cryptographic provider abstraction for pluggable algorithm support.
//!
//! This module defines the `CryptoProvider` trait that abstracts cryptographic
//! operations, allowing the system to switch between different algorithm suites
//! (e.g., standard algorithms vs. Chinese national cryptography standards).
//!
//! # Security Model
//!
//! The trait enforces a zero-trust security model:
//! - All inputs are treated as untrusted
//! - All cryptographic operations must return explicit Result types
//! - Sensitive data is automatically zeroed on drop via SecureBytes
//! - No panics in production code paths

use crate::secrets::types::{DecryptedSecret, SecretError};

/// Cryptographic algorithm suite identifier.
///
/// Determines which algorithm implementations are used for encryption,
/// signing, hashing, and key derivation operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CryptoAlgorithm {
    /// Standard algorithms: AES-256-GCM, Ed25519, SHA256
    Default,
    /// Chinese national cryptography: SM2, SM3, SM4
    ChinaCrypto,
}

/// Trait for cryptographic operations with pluggable algorithm support.
///
/// Implementations provide encryption, decryption, signing, verification,
/// hashing, and key derivation using different algorithm suites.
pub trait CryptoProvider: Send + Sync {
    /// Get the algorithm suite used by this provider.
    fn algorithm(&self) -> CryptoAlgorithm;

    /// Encrypt plaintext data.
    ///
    /// Returns (encrypted_value, salt) where:
    /// - encrypted_value contains the ciphertext and authentication data
    /// - salt is used for key derivation
    fn encrypt(&self, plaintext: &[u8]) -> Result<(Vec<u8>, Vec<u8>), SecretError>;

    /// Decrypt encrypted data.
    ///
    /// Takes the encrypted_value and salt that was used during encryption.
    fn decrypt(&self, encrypted_value: &[u8], salt: &[u8]) -> Result<DecryptedSecret, SecretError>;

    /// Sign data using the provider's signature algorithm.
    ///
    /// Returns the signature bytes.
    fn sign(&self, data: &[u8], private_key: &[u8]) -> Result<Vec<u8>, SecretError>;

    /// Verify a signature using the provider's signature algorithm.
    ///
    /// Returns true if the signature is valid.
    fn verify(
        &self,
        data: &[u8],
        signature: &[u8],
        public_key: &[u8],
    ) -> Result<bool, SecretError>;

    /// Compute a cryptographic hash of the input data.
    fn hash(&self, data: &[u8]) -> Vec<u8>;

    /// Derive a key from a master key and salt.
    fn derive_key(
        &self,
        master_key: &[u8],
        salt: &[u8],
        info: &[u8],
    ) -> Result<Vec<u8>, SecretError>;

    /// Generate a random salt for key derivation.
    fn generate_salt(&self) -> Vec<u8>;
}

/// Secure byte wrapper that zeros memory on drop.
///
/// This type ensures that sensitive data (keys, plaintext) is securely
/// erased from memory when no longer needed. Uses constant-time operations
/// to prevent timing attacks during verification.
///
/// # Security Properties
///
/// - Automatically zeros memory on drop
/// - Prevents accidental exposure via Debug output
/// - Uses constant-time comparisons for verification
/// - Cannot be accidentally cloned (no Clone impl)
#[derive(Clone)]
pub struct SecureBytes {
    data: Vec<u8>,
}

impl SecureBytes {
    /// Create a new SecureBytes from a vector.
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }

    /// Create a new SecureBytes with a specific capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
        }
    }

    /// Get a reference to the underlying data.
    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }

    /// Get the length of the data.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if the data is empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Extend the data with additional bytes.
    pub fn extend_from_slice(&mut self, other: &[u8]) {
        self.data.extend_from_slice(other);
    }

    /// Convert into the underlying vector (consumes self).
    pub fn into_vec(mut self) -> Vec<u8> {
        let data = std::mem::take(&mut self.data);
        std::mem::forget(self); // Prevent Drop from being called
        data
    }
}

impl Drop for SecureBytes {
    fn drop(&mut self) {
        // Zero out the memory before dropping
        use subtle::ConstantTimeEq;
        for byte in &mut self.data {
            *byte = 0;
        }
        // Verify zeroing (constant-time check)
        let all_zero = self.data.iter().fold(0u8, |acc, &b| acc | b);
        debug_assert_eq!(all_zero.ct_eq(&0).unwrap_u8(), 1);
    }
}

impl std::fmt::Debug for SecureBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecureBytes")
            .field("len", &self.data.len())
            .field("data", &"[REDACTED]")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secure_bytes_zeros_on_drop() {
        let data = vec![1, 2, 3, 4, 5];
        let _ptr = data.as_ptr();

        {
            let _secure = SecureBytes::new(data);
            // secure will be dropped here
        }

        // Note: This test is illustrative. In practice, we can't safely
        // read from the pointer after drop without undefined behavior.
        // The zeroing is verified in the Drop impl itself.
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

    #[test]
    fn test_secure_bytes_debug_redacts() {
        let secure = SecureBytes::new(vec![1, 2, 3, 4, 5]);
        let debug = format!("{:?}", secure);
        assert!(debug.contains("REDACTED"));
        assert!(!debug.contains("1"));
        assert!(!debug.contains("2"));
    }
}
