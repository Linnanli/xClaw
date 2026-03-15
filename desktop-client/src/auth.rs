use crate::{Error, Result};
use argon2::{Argon2, PasswordHasher, PasswordHash, PasswordVerifier};
use argon2::password_hash::SaltString;
use rand::Rng;
use sha2::{Sha256, Digest};
use std::time::{SystemTime, UNIX_EPOCH};

const MIN_PASSWORD_LENGTH: usize = 12;
const PASSWORD_STRENGTH_THRESHOLD: u32 = 3;
const SESSION_TIMEOUT_SECS: u64 = 30 * 60; // 30 minutes

#[derive(Debug, Clone)]
pub struct MasterPassword {
    hash: String,
    salt: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct Session {
    pub user_id: String,
    pub created_at: u64,
    pub last_activity: u64,
}

impl Session {
    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        now - self.last_activity > SESSION_TIMEOUT_SECS
    }

    pub fn update_activity(&mut self) {
        self.last_activity = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }
}

pub struct AuthManager {
    master_password: Option<MasterPassword>,
    session: Option<Session>,
}

impl AuthManager {
    pub fn new() -> Self {
        Self {
            master_password: None,
            session: None,
        }
    }

    pub fn setup_master_password(&mut self, password: &str) -> Result<()> {
        // Validate password strength
        self.validate_password_strength(password)?;

        // Generate salt
        let salt = SaltString::generate(rand::thread_rng());

        // Hash password using Argon2
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| Error::CryptoError(e.to_string()))?
            .to_string();

        self.master_password = Some(MasterPassword {
            hash: password_hash,
            salt: salt.as_str().as_bytes().to_vec(),
        });

        Ok(())
    }

    pub fn verify_password(&self, password: &str) -> Result<()> {
        let master_pwd = self
            .master_password
            .as_ref()
            .ok_or(Error::AuthError("Master password not set".to_string()))?;

        let parsed_hash = PasswordHash::new(&master_pwd.hash)
            .map_err(|e| Error::CryptoError(e.to_string()))?;

        let argon2 = Argon2::default();
        argon2
            .verify_password(password.as_bytes(), &parsed_hash)
            .map_err(|_| Error::InvalidPassword)?;

        Ok(())
    }

    pub fn create_session(&mut self, user_id: String) -> Result<Session> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let session = Session {
            user_id,
            created_at: now,
            last_activity: now,
        };

        self.session = Some(session.clone());
        Ok(session)
    }

    pub fn get_session(&self) -> Result<&Session> {
        let session = self
            .session
            .as_ref()
            .ok_or(Error::Unauthorized)?;

        if session.is_expired() {
            return Err(Error::SessionExpired);
        }

        Ok(session)
    }

    pub fn update_session_activity(&mut self) -> Result<()> {
        if let Some(session) = &mut self.session {
            session.update_activity();
        }
        Ok(())
    }

    pub fn logout(&mut self) {
        self.session = None;
    }

    fn validate_password_strength(&self, password: &str) -> Result<()> {
        if password.len() < MIN_PASSWORD_LENGTH {
            return Err(Error::AuthError(format!(
                "Password must be at least {} characters",
                MIN_PASSWORD_LENGTH
            )));
        }

        let mut strength = 0u32;
        if password.chars().any(|c| c.is_lowercase()) {
            strength += 1;
        }
        if password.chars().any(|c| c.is_uppercase()) {
            strength += 1;
        }
        if password.chars().any(|c| c.is_numeric()) {
            strength += 1;
        }
        if password.chars().any(|c| !c.is_alphanumeric()) {
            strength += 1;
        }

        if strength < PASSWORD_STRENGTH_THRESHOLD {
            return Err(Error::AuthError(
                "Password must contain uppercase, lowercase, numbers, and special characters"
                    .to_string(),
            ));
        }

        Ok(())
    }

    pub fn derive_key(&self, password: &str, salt: &[u8]) -> Result<Vec<u8>> {
        use hkdf::Hkdf;

        let hkdf = Hkdf::<Sha256>::new(Some(salt), password.as_bytes());
        let mut key = vec![0u8; 32];
        hkdf.expand(b"ironclaw-desktop-key", &mut key)
            .map_err(|e| Error::CryptoError(e.to_string()))?;

        Ok(key)
    }
}

impl Default for AuthManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_password_strength_validation() {
        let mut auth = AuthManager::new();

        // Too short
        assert!(auth.setup_master_password("Short1!").is_err());

        // Only 2 categories (lower + numbers), threshold is 3
        assert!(auth.setup_master_password("validpassword123").is_err());

        // Valid password (has upper, lower, numbers = 3 categories, meets threshold of 3)
        assert!(auth.setup_master_password("ValidPassword123").is_ok());
    }

    #[test]
    fn test_password_verification() {
        let mut auth = AuthManager::new();
        auth.setup_master_password("ValidPassword123!").unwrap();

        assert!(auth.verify_password("ValidPassword123!").is_ok());
        assert!(auth.verify_password("WrongPassword123!").is_err());
    }

    #[test]
    fn test_session_management() {
        let mut auth = AuthManager::new();
        let session = auth.create_session("user123".to_string()).unwrap();

        assert_eq!(session.user_id, "user123");
        assert!(!session.is_expired());

        let retrieved = auth.get_session().unwrap();
        assert_eq!(retrieved.user_id, "user123");

        auth.logout();
        assert!(auth.get_session().is_err());
    }
}
