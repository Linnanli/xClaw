use crate::error::{Error, Result};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use argon2::password_hash::SaltString;
use rand::Rng;

pub struct PasswordManager;

impl PasswordManager {
    pub fn new() -> Self {
        Self
    }

    pub fn hash(&self, password: &str) -> Result<String> {
        let salt = SaltString::generate(rand::thread_rng());
        let argon2 = Argon2::default();

        argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| Error::AuthFailed(e.to_string()))
            .map(|hash| hash.to_string())
    }

    pub fn verify(&self, password: &str, hash: &str) -> Result<()> {
        let parsed_hash = PasswordHash::new(hash)
            .map_err(|e| Error::AuthFailed(e.to_string()))?;

        let argon2 = Argon2::default();
        argon2
            .verify_password(password.as_bytes(), &parsed_hash)
            .map_err(|_| Error::InvalidCredentials)
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
    fn test_hash_and_verify() {
        let manager = PasswordManager::new();
        let password = "secure-password-123";

        let hash = manager.hash(password).unwrap();
        assert!(manager.verify(password, &hash).is_ok());
    }

    #[test]
    fn test_wrong_password_fails() {
        let manager = PasswordManager::new();
        let password = "secure-password-123";

        let hash = manager.hash(password).unwrap();
        assert!(manager.verify("wrong-password", &hash).is_err());
    }

    #[test]
    fn test_different_hashes_for_same_password() {
        let manager = PasswordManager::new();
        let password = "secure-password-123";

        let hash1 = manager.hash(password).unwrap();
        let hash2 = manager.hash(password).unwrap();

        // Different hashes due to random salt
        assert_ne!(hash1, hash2);
        // But both verify correctly
        assert!(manager.verify(password, &hash1).is_ok());
        assert!(manager.verify(password, &hash2).is_ok());
    }
}
