use crate::error::{AuthError, Result};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use argon2::password_hash::SaltString;
use rand::thread_rng;

/// Password manager using Argon2.
pub struct PasswordManager {
    argon2: Argon2<'static>,
}

impl PasswordManager {
    /// Create a new password manager.
    pub fn new() -> Self {
        Self {
            argon2: Argon2::default(),
        }
    }

    /// Hash a password using Argon2.
    pub fn hash(&self, password: &str) -> Result<String> {
        let salt = SaltString::generate(thread_rng());

        self.argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| AuthError::AuthFailed(e.to_string()))
            .map(|hash| hash.to_string())
    }

    /// Verify a password against a hash.
    pub fn verify(&self, password: &str, hash: &str) -> Result<()> {
        let parsed_hash = PasswordHash::new(hash)
            .map_err(|e| AuthError::AuthFailed(e.to_string()))?;

        self.argon2
            .verify_password(password.as_bytes(), &parsed_hash)
            .map_err(|_| AuthError::InvalidCredentials)
    }
}

impl Default for PasswordManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_and_verify_password() {
        let manager = PasswordManager::new();
        let password = "secure-password-123";

        let hash = manager.hash(password).unwrap();
        assert!(manager.verify(password, &hash).is_ok());
        assert!(manager.verify("wrong-password", &hash).is_err());
    }

    #[test]
    fn test_different_salts_produce_different_hashes() {
        let manager = PasswordManager::new();
        let password = "same-password";

        let hash1 = manager.hash(password).unwrap();
        let hash2 = manager.hash(password).unwrap();

        assert_ne!(hash1, hash2);
        assert!(manager.verify(password, &hash1).is_ok());
        assert!(manager.verify(password, &hash2).is_ok());
    }
}
